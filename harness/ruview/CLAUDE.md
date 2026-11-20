# RuView harness — agent operating notes

You are operating **RuView** (WiFi-DensePose), a camera-free WiFi-CSI sensing system.

## The one rule: prove everything

This project was accused of AI-slop; the fix is hard discipline. Before you quote ANY
accuracy number:

1. It must be tagged **MEASURED** (with a reproducer named), **CLAIMED**, or **SYNTHETIC**.
2. Pose PCK is quoted only as a **delta over the mean-pose baseline** on a leakage-free
   held-out split; that baseline can otherwise make an unusable model look strong.
3. Run `ruview_claim_check` on any report/PR/model-card. It flags untagged numbers and
   the project's retracted perfect-accuracy framing.
4. Firmware is "hardware-validated" only with a captured **boot log on real silicon** —
   never on a build-passes signal.

## Tools

`ruview_onboard`, `ruview_claim_check`, `ruview_verify`, `ruview_node_monitor`,
`ruview_calibrate`, `ruview_node_flash`, `ruview_guidance`,
`ruview_spaces_list`, `ruview_memory_search`, `ruview_kernel_selftest`,
`ruview_doctor`, `ruview_firmware_plan`, `ruview_firmware_ports`,
`ruview_train`, `ruview_train_plan`, `ruview_train_gate`,
`ruview_devices_scan`, `ruview_esp32_capture`, `ruview_mmwave_read`,
`ruview_lidar_read`, `ruview_host_list`, `ruview_host_run`. Device tools need
the `device-access` MCP grant; remote hosts need `remote-host`. Start
unfamiliar work with
`ruview_guidance`; its
capability status, source paths, validation commands, and limitations are
navigation evidence, not authority. All tools fail closed. Mutating/hardware
tools (`node_flash`) require explicit confirmation and are Windows/ESP-IDF
gated.

`ruview_spaces_list` is an OAuth-only external read for the eight versioned
hierarchy/event/alert collections. MCP calls require the
`credential-use` grant, cannot select a credential path or API origin, and may
rotate the local refresh credential. It requires an installed binary and never
runs Cargo from an auto-detected checkout. Cursors are opaque and collection-
bound. It grants no write or action authority.

## Skills

`onboard` · `provision-node` · `calibrate-room` · `train-pose` · `verify` · `cognitum-spaces`
(`npx @ruvnet/ruview skill <name>`).

## Don'ts

- Don't present WiFi sensing as camera-grade.
- Don't echo or commit WiFi passwords / secrets.
- Don't merge or release firmware without a real boot log.
- Don't report a PCK without its mean-pose baseline.
