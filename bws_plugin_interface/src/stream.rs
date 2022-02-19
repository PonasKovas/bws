use safe_types::std::task::SContext;

// /// A wrapper around a TCP stream to a client
// #[repr(C)]
// pub struct Stream<'a> {
//     inner: &'a (), // ptr to TcpStream
//     vtable: &'static StreamVTable,
// }

// #[repr(C)]
// pub struct StreamVTable {
//     poll_send_packet:
//         unsafe extern "C" fn(&(), &mut SContext, &ClientBound) -> SPoll<SResult<SUnit, SUnit>>,
// }
