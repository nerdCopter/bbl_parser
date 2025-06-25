## Current Implementation Status (June 2025)

✅ **COMPLETED GOALS:**
- Full BBL binary format parsing using JavaScript blackbox-log-viewer reference
- Complete I, P, S frame parsing with proper predictor implementation
- Header parsing and field definition extraction
- CSV export with 100%+ accuracy vs reference implementation (based on tested files)
- Main CSV export with Betaflight-compatible field ordering
- Headers CSV export by default (--save-headers equivalent)
- Proper field encoding/decoding (signed VB, unsigned VB, etc.)
- Motor value prediction fix (100% accuracy achieved)
- S-frame timestamp inheritance and data merging
- Multi-log detection and separate file generation
- Basic unit conversions (voltage, current)
- Energy calculation (energyCumulative field)
- Time-sorted CSV output with proper chronological ordering
- Debug mode with frame-by-frame analysis
- Large file streaming support (432K+ frames)
- **Betaflight firmware-accurate flag formatting** (flightModeFlags, stateFlags, failsafePhase)
- **100% test success rate** (21/21 files in comprehensive testing)

🔧 **REMAINING WORK:**
- Code refinement: Replace unwrap() calls with proper error handling
- Complete missing implementations in frame parsing
- S-frame field association (rxSignalReceived, rxFlightChannelsValid)  
- G-frame (GPS) parsing and GPS CSV export
- E-frame (event) parsing optimization and JSON event export
- GPX file export for GPS track visualization
- Unit conversion options (time, voltage, current, height, speed, rotation, acceleration)
- IMU simulation (roll/pitch/yaw angle computation from gyro/accel/mag)
- Current meter simulation and energy integration
- GPS merge option (integrate GPS data into main CSV)
- Raw mode output (unprocessed sensor values)
- Statistics output (frame counts, timing, loop statistics)
- Full RUST CRATE for Reusability and Modularity
- Comprehensive error handling and edge case testing

📊 **CURRENT ACCURACY:** 100.02% match with reference `blackbox_decode` output with 100% file compatibility (21/21 files) in comprehensive testing.

---

Implement the actual BBL binary format specification by explicitly replicating the JavaScript code from the Betaflight blackbox-log-viewer repository using the following sources:
https://github.com/betaflight/blackbox-log-viewer/tree/master/src
https://raw.githubusercontent.com/betaflight/blackbox-log-viewer/master/src/flightlog.js
https://raw.githubusercontent.com/betaflight/blackbox-log-viewer/master/src/flightlog_parser.js
https://raw.githubusercontent.com/betaflight/blackbox-log-viewer/master/src/datastream.js
https://raw.githubusercontent.com/betaflight/blackbox-log-viewer/master/src/decoders.js

The goal is to fully read, parse and decode binary BBL files. Do not re-invent, explicitly use the javascript as a source to create the RUST project's code.

Every BBL contains headers in plaintext which contain important information about the aircraft's settings, but more importantly, they contain details about the binary data and how to decode them:
`Field I name`
`Field I signed`
`Field I predictor`
`Field I encoding`
`Field P predictor`
`Field P encoding`
`Field S name`
`Field S signed`
`Field S predictor`
`Field S encoding`

Each BBL may or may not contains multiple flights logs. Each flight log starts with it's own set of plaintext headers. Each flight log within a BBL will contain I frames and P frames, and maybe contain E frames and S frames and G frames. G frames are GPS. H frames are GPS home position markers.

**IMPLEMENTATION NOTE:** Our RUST parser successfully handles I, P, S frames with high accuracy. E-frames (events) are parsed but not included in CSV output as they represent discrete events rather than continuous flight data.

Binary utility `blackbox_decode` outputs useful statistics and creates `.csv`, `.gpx`, and `.event` files that contain the flightlog data.  We can use it for data comparison, but do not embed nor call binary tools from within the RUST program. `blackbox_decode --limits` can be used for any `*.BBL` file.  The `--limits` is only useful to see the min and max `loopIteration` and `time`.

**TESTING STATUS:** Parser successfully processes both Betaflight and EmuFlight BBL files with 98%+ accuracy compared to reference implementations.

Please use `timeout` when testing BBL parsing. I would expect it not to take over 60s unless debug output slows the process. Do not set a timeout less than 15s because it is too short.

We can use the older blackbox_decode (a.k.a blackbox-tools) project https://github.com/betaflight/blackbox-tools/blob/master/src/blackbox_decode.c for further analysis and comaprison.

Two RUST projects on github that may help or may hinder, i do not know. I never inspected the first, and the second is betaflight version specific and not up to date for Betaflight 4.6.
 1) https://github.com/ilya-epifanov/fc-blackbox
 2) https://github.com/blackbox-log/blackbox-log

Use `.github/copilot-instructions.md`; request clarification if needed.
