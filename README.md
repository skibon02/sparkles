# Sparkles: tracing library

### Implementation status

| Requirement                                                                                                                 | Status |
|-----------------------------------------------------------------------------------------------------------------------------|--------|
| **Runtime Requirements (average x86 pc)**                                                                                   |        |
| 1. "Add trace event" single call duration <30ns                                                                             | 🟢     |
| 2. Events flow up to 40kk/s in long run with no data loss (localhost environment / IPC)                                     | 🟢     |
| 3. Events flow up to 10kk/s in long run with no data loss (real conditions: ethernet/wifi)                                  | 🟡     |
| 4. Capture and transfer loss detection with no corruption to other captured and transmitted data                            | 🟡     |
| 5. Configurable limit of memory consumption for trace buffer in capturing client (static/dynamic with limit)                | 🟡     |
| **Other requirements**                                                                                                      |        |
| 1. Event name dynamic encoding: send literal id instead of string data. Should work "on the fly" without predefined mapping | 🟡     |
| 2. Additional simple data, attached to event. (list of integers)                                                            | 🔴     |
| 3. Additional `std::fmt::Debug` data, represented as string. No encoding here.                                              | 🔴     |
| 4. Ranges support: simple, multiplexed (start, and one of predefined end states)                                            | 🔴     |
| 5. Module info support: full module path, line of code                                                                      | 🔴     |
| 6. Abstraction over captured events transfer type (UDP/save to file)                                                        | 🔴     |
| 7. Filtering on receiving side                                                                                              | 🔴     |
| 8. Viewer memory limit: dynamically save/load to/from files                                                                 | 🔴     |
| 9. Multi-app sync                                                                                                           | 🔴     |
| 10. Global ranges                                                                                                           | 🔴     |
| 11. Measuring overhead self-test                                                                                            | 🔴     |

- 🔴 Requirement Not Met
- 🟡 Work in Progress
- 🟢 Done