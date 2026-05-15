# RuView -> Home Assistant MQTT Bridge

Subscribes to `sensing-server`'s WebSocket (`/ws/sensing`), translates each
`SensingUpdate` into MQTT topics, and publishes Home Assistant MQTT
**Discovery** configs so HA auto-creates entities (presence, fall, vitals,
person count, per-node RSSI, ...) without any YAML edits.

## Environment variables

| Variable               | Default                                 | Notes                                              |
|------------------------|-----------------------------------------|----------------------------------------------------|
| `RUVIEW_WS`            | `ws://sensing-server:3001/ws/sensing`   | Sensing server WebSocket URL                       |
| `MQTT_HOST`            | `homeassistant.local`                   | MQTT broker host (Mosquitto add-on, etc.)          |
| `MQTT_PORT`            | `1883`                                  |                                                    |
| `MQTT_USER` / `MQTT_PASS` | (empty)                              | Set when the broker requires auth                  |
| `MQTT_PREFIX`          | `ruview`                                | Topic root (`<prefix>/state`, `<prefix>/event`, ...) |
| `HA_DISCOVERY_PREFIX`  | `homeassistant`                         | Match HA's MQTT integration setting                |
| `DEVICE_NAME`          | `RuView Sensing`                        | Friendly name in HA                                |
| `DEVICE_ID`            | `ruview`                                | Unique device id (also used in entity unique_ids)  |
| `LOG_LEVEL`            | `INFO`                                  | `DEBUG` for full WS frame tracing                  |

## Topics published

| Topic                              | Retained | Payload                                         |
|------------------------------------|----------|-------------------------------------------------|
| `<prefix>/availability`            | yes      | `online` / `offline` (LWT)                       |
| `<prefix>/state`                   | no       | Flat JSON consumed by all entity templates       |
| `<prefix>/nodes/<id>`              | no       | Per-node RSSI / position                         |
| `<prefix>/event`                   | no       | One-shot fall events (HA `event` entity)         |
| `homeassistant/<comp>/<id>/.../config` | yes  | HA Discovery configs                             |

## Run standalone

```bash
docker run --rm \
  -e MQTT_HOST=192.168.1.10 \
  -e MQTT_USER=ha -e MQTT_PASS=secret \
  -e RUVIEW_WS=ws://192.168.1.20:3001/ws/sensing \
  ruvnet/ruview-mqtt-bridge:latest
```

Inside the project's `docker-compose.yml`, the bridge is wired to the
internal network and the WS URL defaults to `sensing-server:3001`.
