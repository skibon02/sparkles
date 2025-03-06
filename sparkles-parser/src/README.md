# Sparkles-parser
This package will parse sparkles data, which exists in two flavors:
- Stream: Byte stream with guaranteed order and delivery (file, TCP)
- Packets: Packets of fixed size but without delivery and order guarantees. However, packet size is guaranteed to not be altered.

## ✧ Binaries
Two binaries are included in this package:
- **[sparkles-parse-and-save]** Read the latest trace file in the `trace` folder and convert it to the Perfetto format. Converted file can be directly dropped to [perfetto](ui.perfetto.dev) for viewing timestamp events directly in your browser.
- **[sparkles-udp-parse-and-save]** Udp version of the previous binary. Will wait for connection, receive trace data over UDP, and save it to Perfetto format when the application closes.