// ============================================================
// GOS KERNEL TOPOLOGY — k-net::tcp
//
// MERGE (m:Module {id: "K_NET_TCP", name: "k-net::tcp"})
// SET m.role = "transport", m.responsibility = "minimal happy-path TCP/HTTP client"
// MERGE (p:Plugin {id: "K_NET"})
// MERGE (m)-[:BELONGS_TO]->(p)
// ============================================================
//
// Implements a minimal TCP client sufficient for HTTP/1.0 POST to a local
// host (QEMU SLIRP gateway 10.0.2.2).  No retransmit, no fragmentation,
// no out-of-order delivery — this is a single-segment, blocking happy-path
// implementation suited for cooperative uniprocessor kernel use.
//
// Frame layout (offsets in TX_BUF / RX frames):
//   0  ..  5  — Ethernet dst MAC
//   6  .. 11  — Ethernet src MAC
//  12  .. 13  — EtherType (0x0800 IPv4)
//  14  .. 33  — IPv4 header (20 bytes, IHL=5)
//  34  .. 53  — TCP header  (20 bytes, data-offset=5, no options)
//  54  ..     — TCP payload

use core::hint::spin_loop;

use super::{
    NetState, GUEST_IP, PACKET_SIZE, STAGE_DEVICE_READY,
    arp_request, arp_wait_reply, tx_send, rx_poll,
    checksum_ip, write_u16_be, TX_BUF, GATEWAY_IP,
    e1000_ring_init,
};

// ── TCP flag bits ────────────────────────────────────────────────────────────
const TCP_FIN: u8 = 0x01;
const TCP_SYN: u8 = 0x02;
#[allow(dead_code)]
const TCP_RST: u8 = 0x04;
const TCP_PSH: u8 = 0x08;
const TCP_ACK: u8 = 0x10;

// ── Connection parameters ────────────────────────────────────────────────────
const LOCAL_PORT: u16    = 49_200;
const ISN:        u32    = 0x1234_5678;  // initial sequence number
const TCP_WINDOW: u16    = 8_192;

/// Maximum TCP payload in a single segment (Ethernet MTU 1500 − 40 bytes hdr).
pub(crate) const TCP_MAX_PAYLOAD: usize = PACKET_SIZE - 54; // 1468

/// Busy-poll iterations allowed for receiving one TCP segment (~5 s equivalent).
const RECV_TIMEOUT: usize = 5_000_000;
/// Total iterations for the full response collection loop (~30 s equivalent).
const HTTP_TIMEOUT: usize = 60_000_000;

// ── TCP connection context ────────────────────────────────────────────────────

struct TcpConn {
    src_port: u16,
    dst_port: u16,
    dst_ip:   [u8; 4],
    dst_mac:  [u8; 6],
    seq:      u32,
    ack:      u32,
}

// ── TCP pseudo-header checksum ────────────────────────────────────────────────

/// Internet checksum over the TCP pseudo-header + TCP segment.
///
/// Pseudo-header layout:  src_ip (4) | dst_ip (4) | zero (1) | proto=6 (1) | tcp_len (2)
fn tcp_checksum(src_ip: [u8; 4], dst_ip: [u8; 4], tcp_seg: &[u8]) -> u16 {
    let tcp_len = tcp_seg.len() as u32;
    let mut sum = 0u32;

    // Pseudo-header
    sum += u32::from(u16::from_be_bytes([src_ip[0], src_ip[1]]));
    sum += u32::from(u16::from_be_bytes([src_ip[2], src_ip[3]]));
    sum += u32::from(u16::from_be_bytes([dst_ip[0], dst_ip[1]]));
    sum += u32::from(u16::from_be_bytes([dst_ip[2], dst_ip[3]]));
    sum += 6u32;        // protocol = TCP (upper byte is 0x00)
    sum += tcp_len;     // TCP segment length

    // TCP segment bytes
    let mut i = 0usize;
    while i + 1 < tcp_seg.len() {
        sum += u32::from(u16::from_be_bytes([tcp_seg[i], tcp_seg[i + 1]]));
        i += 2;
    }
    if i < tcp_seg.len() {
        sum += u32::from(tcp_seg[i]) << 8;
    }

    while sum >> 16 != 0 {
        sum = (sum & 0xFFFF) + (sum >> 16);
    }
    !(sum as u16)
}

// ── Frame builder ────────────────────────────────────────────────────────────

/// Write an Ethernet + IPv4 + TCP frame into `TX_BUF` with `data` as payload.
/// Returns the total frame length written.
///
/// # Safety
/// Caller must ensure exclusive access to `TX_BUF` and that `data.len() ≤ TCP_MAX_PAYLOAD`.
unsafe fn build_tcp_frame(state: &NetState, conn: &TcpConn, flags: u8, data: &[u8]) -> usize {
    debug_assert!(data.len() <= TCP_MAX_PAYLOAD);
    let buf = unsafe { &mut *TX_BUF.0.get() };
    let src_mac = state.mac;
    let src_ip  = GUEST_IP;

    // ── Ethernet header (14 bytes) ───────────────────────────────────────────
    buf[0..6].copy_from_slice(&conn.dst_mac);
    buf[6..12].copy_from_slice(&src_mac);
    write_u16_be(buf, 12, 0x0800); // EtherType = IPv4

    // ── IPv4 header (20 bytes @ offset 14) ──────────────────────────────────
    let tcp_seg_len = 20 + data.len();          // TCP hdr + payload
    let ip_total    = (20 + tcp_seg_len) as u16; // IP hdr + TCP
    buf[14] = 0x45; // version=4, IHL=5
    buf[15] = 0x00; // DSCP / ECN
    write_u16_be(buf, 16, ip_total);
    write_u16_be(buf, 18, 0x1234); // ID (constant; we're one-shot)
    write_u16_be(buf, 20, 0x4000); // flags: don't fragment
    buf[22] = 64;   // TTL
    buf[23] = 6;    // protocol = TCP
    write_u16_be(buf, 24, 0); // checksum placeholder
    buf[26..30].copy_from_slice(&src_ip);
    buf[30..34].copy_from_slice(&conn.dst_ip);
    let ip_csum = checksum_ip(&buf[14..34]);
    write_u16_be(buf, 24, ip_csum);

    // ── TCP header (20 bytes @ offset 34) ───────────────────────────────────
    write_u16_be(buf, 34, conn.src_port);
    write_u16_be(buf, 36, conn.dst_port);
    // Sequence number (big-endian)
    buf[38] = (conn.seq >> 24) as u8;
    buf[39] = (conn.seq >> 16) as u8;
    buf[40] = (conn.seq >>  8) as u8;
    buf[41] =  conn.seq        as u8;
    // Acknowledgement number
    buf[42] = (conn.ack >> 24) as u8;
    buf[43] = (conn.ack >> 16) as u8;
    buf[44] = (conn.ack >>  8) as u8;
    buf[45] =  conn.ack        as u8;
    buf[46] = 0x50; // data offset = 5 (20 bytes), no options
    buf[47] = flags;
    write_u16_be(buf, 48, TCP_WINDOW);
    write_u16_be(buf, 50, 0); // checksum placeholder
    write_u16_be(buf, 52, 0); // urgent pointer

    // ── Payload ──────────────────────────────────────────────────────────────
    if !data.is_empty() {
        buf[54..54 + data.len()].copy_from_slice(data);
    }

    // TCP checksum covers pseudo-header + TCP header + payload
    let tcp_csum = tcp_checksum(src_ip, conn.dst_ip, &buf[34..54 + data.len()]);
    write_u16_be(buf, 50, tcp_csum);

    54 + data.len()
}

// ── Frame receiver ───────────────────────────────────────────────────────────

/// Returned when a matching TCP segment arrives.
#[allow(dead_code)]
struct TcpSegment {
    flags:       u8,
    seq:         u32,
    ack:         u32,
    payload_off: usize, // byte offset into `frame` where payload starts
    payload_len: usize,
}

/// Poll RX ring until a TCP segment from the peer (conn.dst_ip:dst_port →
/// our src_port) is received, or `max_polls` is exhausted.
///
/// Returns `None` on timeout.
///
/// # Safety
/// Caller must hold exclusive access to the NIC ring.
unsafe fn recv_tcp(
    state: &mut NetState,
    frame: &mut [u8; PACKET_SIZE],
    conn:  &TcpConn,
    max_polls: usize,
) -> Option<TcpSegment> {
    let mut polls = 0usize;
    while polls < max_polls {
        if let Some(len) = unsafe { rx_poll(state, frame) } {
            // Need at least Ethernet(14) + IP(20) + TCP(20) = 54 bytes
            if len >= 54 && frame[12] == 0x08 && frame[13] == 0x00 && frame[23] == 6 {
                // Source IP must match peer
                if frame[26..30] == conn.dst_ip {
                    let src_port = u16::from_be_bytes([frame[34], frame[35]]);
                    let dst_port = u16::from_be_bytes([frame[36], frame[37]]);
                    if src_port == conn.dst_port && dst_port == conn.src_port {
                        let seq = u32::from_be_bytes([frame[38], frame[39], frame[40], frame[41]]);
                        let ack = u32::from_be_bytes([frame[42], frame[43], frame[44], frame[45]]);
                        let flags = frame[47];
                        let data_offset = ((frame[46] >> 4) * 4) as usize;
                        let ip_ihl      = ((frame[14] & 0x0F) * 4) as usize;
                        let payload_off = 14 + ip_ihl + data_offset;
                        let ip_total    = u16::from_be_bytes([frame[16], frame[17]]) as usize;
                        let payload_len = ip_total
                            .saturating_sub(ip_ihl + data_offset);
                        let payload_len = payload_len.min(
                            len.saturating_sub(payload_off),
                        );
                        return Some(TcpSegment { flags, seq, ack, payload_off, payload_len });
                    }
                }
            }
        }
        spin_loop();
        polls += 1;
    }
    None
}

// ── HTTP/1.0 POST over TCP ────────────────────────────────────────────────────

/// Execute a single HTTP/1.0 POST over a fresh TCP connection to
/// `dst_ip:dst_port`.  The caller provides the **complete** HTTP request
/// (headers + blank line + body) in `request`.  The raw HTTP response
/// (status line + headers + body) is written into `resp` and its length is
/// returned.
///
/// Returns `None` if the NIC is not ready, ARP fails, TCP handshake fails, or
/// the receive loop times out with zero data.
///
/// # Safety
/// Caller must ensure the NIC is not in use by any other operation at the same
/// time (trivially satisfied in GOS's cooperative uniprocessor model).
pub(crate) unsafe fn tcp_http_post(
    state:    &mut NetState,
    dst_ip:   [u8; 4],
    dst_port: u16,
    request:  &[u8],
    resp:     &mut [u8],
) -> Option<usize> {
    if state.stage != STAGE_DEVICE_READY || state.mac_valid == 0 {
        return None;
    }
    if state.ring_initialized == 0 {
        e1000_ring_init(state);
        if state.ring_initialized == 0 {
            return None;
        }
    }
    if request.len() > TCP_MAX_PAYLOAD {
        return None; // request too large for a single segment
    }

    // ── Step 1: Resolve gateway MAC (ARP cache) ──────────────────────────────
    let gw_mac = if state.gw_mac_valid != 0 {
        state.gw_mac
    } else {
        if !unsafe { arp_request(state, GATEWAY_IP) } {
            return None;
        }
        let mut mac = [0u8; 6];
        if !unsafe { arp_wait_reply(state, GATEWAY_IP, &mut mac, 2_000_000) } {
            return None;
        }
        state.gw_mac     = mac;
        state.gw_mac_valid = 1;
        mac
    };

    let mut conn = TcpConn {
        src_port: LOCAL_PORT,
        dst_port,
        dst_ip,
        dst_mac: gw_mac,
        seq: ISN,
        ack: 0,
    };

    let mut frame = [0u8; PACKET_SIZE];

    // ── Step 2: SYN ─────────────────────────────────────────────────────────
    {
        let len = unsafe { build_tcp_frame(state, &conn, TCP_SYN, &[]) };
        if !unsafe { tx_send(state, len) } { return None; }
    }
    conn.seq = conn.seq.wrapping_add(1); // SYN consumes one sequence number

    // ── Step 3: Wait for SYN-ACK ────────────────────────────────────────────
    let seg = unsafe { recv_tcp(state, &mut frame, &conn, RECV_TIMEOUT) }?;
    if seg.flags & (TCP_SYN | TCP_ACK) != (TCP_SYN | TCP_ACK) {
        return None; // not SYN-ACK — abort
    }
    conn.ack = seg.seq.wrapping_add(1); // acknowledge server's ISN

    // ── Step 4: ACK (complete 3-way handshake) ───────────────────────────────
    {
        let len = unsafe { build_tcp_frame(state, &conn, TCP_ACK, &[]) };
        if !unsafe { tx_send(state, len) } { return None; }
    }

    // ── Step 5: PSH+ACK with HTTP request ────────────────────────────────────
    {
        let len = unsafe { build_tcp_frame(state, &conn, TCP_PSH | TCP_ACK, request) };
        if !unsafe { tx_send(state, len) } { return None; }
    }
    conn.seq = conn.seq.wrapping_add(request.len() as u32);

    // ── Step 6: Collect response ─────────────────────────────────────────────
    let mut resp_len  = 0usize;
    let mut got_fin   = false;
    let mut polls     = 0usize;

    while polls < HTTP_TIMEOUT && !got_fin {
        if let Some(frame_len) = unsafe { rx_poll(state, &mut frame) } {
            // Filter: IPv4, TCP, from our peer
            if frame_len >= 54
                && frame[12] == 0x08 && frame[13] == 0x00
                && frame[23] == 6
                && frame[26..30] == conn.dst_ip
            {
                let src_p = u16::from_be_bytes([frame[34], frame[35]]);
                let dst_p = u16::from_be_bytes([frame[36], frame[37]]);
                if src_p == conn.dst_port && dst_p == conn.src_port {
                    let seq     = u32::from_be_bytes([frame[38],frame[39],frame[40],frame[41]]);
                    let flags   = frame[47];
                    let d_off   = ((frame[46] >> 4) * 4) as usize;
                    let ip_ihl  = ((frame[14] & 0x0F) * 4) as usize;
                    let pay_off = 14 + ip_ihl + d_off;
                    let ip_tot  = u16::from_be_bytes([frame[16], frame[17]]) as usize;
                    let pay_len = ip_tot.saturating_sub(ip_ihl + d_off)
                        .min(frame_len.saturating_sub(pay_off));

                    // Append payload to response buffer
                    if pay_len > 0 && resp_len < resp.len() {
                        let to_copy = pay_len.min(resp.len() - resp_len);
                        resp[resp_len..resp_len + to_copy]
                            .copy_from_slice(&frame[pay_off..pay_off + to_copy]);
                        resp_len += to_copy;
                        // Advance our ACK to cover the data just received
                        conn.ack = seq.wrapping_add(pay_len as u32);
                        // Send ACK
                        let ack_len = unsafe { build_tcp_frame(state, &conn, TCP_ACK, &[]) };
                        let _ = unsafe { tx_send(state, ack_len) };
                    }

                    // Server closed: FIN flag set
                    if flags & TCP_FIN != 0 {
                        conn.ack = conn.ack.wrapping_add(1); // FIN consumes one seq
                        let fin_len = unsafe { build_tcp_frame(state, &conn, TCP_FIN | TCP_ACK, &[]) };
                        let _ = unsafe { tx_send(state, fin_len) };
                        got_fin = true;
                    }
                }
            }
        }
        spin_loop();
        polls += 1;
    }

    if resp_len == 0 { None } else { Some(resp_len) }
}
