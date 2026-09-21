# Debug Session: basic-rssi-invalid
- **Status**: [OPEN]
- **Issue**: O modo basico via RSSI conecta ao backend e ao Observatory, mas o sinal chega com qualidade invalida ou quase nula (`quality_verdict: Deny`, `signal_quality_score: 0`, `mean_rssi: -100`, `variance: 0`, `motion_band_power: 0`), impedindo deteccao humana confiavel.
- **Debug Server**: http://127.0.0.1:7777/event
- **Log File**: .dbg/trae-debug-log-basic-rssi-invalid.ndjson

## Reproduction Steps
1. Iniciar o RuView em Windows pelo `start.bat` com `--source wifi`.
2. Abrir `http://127.0.0.1:3000/ui/observatory.html`.
3. Observar o painel de sinal e a presenca no modo basico.
4. Comparar os snapshots de `GET /api/v1/sensing/latest` durante o teste.

## Hypotheses & Verification
| ID | Hypothesis | Likelihood | Effort | Evidence |
|----|------------|------------|--------|----------|
| A | O coletor RSSI do Windows nao esta conseguindo ler amostras reais do adaptador e cai em valores sentinela/degenerados. | High | Low | Pending |
| B | O pipeline recebe amostras, mas a filtragem/deduplicacao zera a variancia e o motion score antes da classificacao. | High | Medium | Pending |
| C | O adaptador/rede atual entrega dados somente do AP conectado, com atualizacao lenta ou congelada, abaixo do minimo esperado pelo algoritmo. | Medium | Medium | Pending |
| D | O modo `wifi` esta pegando a interface errada ou um estado stale do Windows, produzindo snapshots validos do ponto de vista tecnico, mas sem relacao com o modem/rede testados. | Medium | Medium | Pending |
| E | A classificacao/adaptive classifier do modo basico esta inferindo `presence` de forma otimista mesmo quando o sinal base esta invalido. | Medium | Low | Pending |

## Log Evidence
Instrumentacao ativa em:
- `main.rs:parse_netsh_interfaces_output` -> hipotese A
- `main.rs:wifi_task` -> hipoteses B/C
- `main.rs:windows_wifi_fallback_tick` -> hipotese D
- `main.rs:derive_pose_from_sensing` -> hipotese E

- Evidencia pre-fix do parser localizado:
  - `.dbg/trae-debug-log-basic-rssi-invalid.ndjson:995-1000` mostrou a transicao de `mean_rssi = -100` para `mean_rssi ~= -57/-58` quando o parser PT-BR passou a entender `Sinal`, `Banda`, `Canal` e `Tipo de Radio`.
- Evidencia de runtime apos reiniciar com o binario atualizado:
  - `.dbg/trae-debug-log-basic-rssi-invalid.ndjson:2245-2264` mostra `observations = 1`, `bssid_count = 3`, `mean_rssi ~= -58`, `quality_verdict = Permit` e `source = wifi:JHON_5G`.
- Snapshot HTTP apos o ajuste de agregacao:
  - `GET /api/v1/sensing/latest` passou a responder com `features.mean_rssi = -59.0` e `nodes[0].rssi_dbm = -59.0`, em vez de propagar `-100`.
- Snapshot HTTP apos o ajuste pratico para hardware comum:
  - `GET /api/v1/sensing/latest` passou a responder com `source = wifi:JHON_5G`, `bssid_count = 1`, `quality_verdict = Warn`, `signal_quality_score ~= 0.25`, `estimated_persons = 1` e `presence = true`.

Correcao minima aplicada apos evidencia:
- `wifi-densepose-wifiscan/src/adapter/netsh_scanner.rs`
  - parser do `netsh wlan show networks mode=bssid` agora aceita rotulos PT-BR.
- `wifi-densepose-sensing-server/src/main.rs`
  - o frame multi-BSSID deixou de herdar cegamente o RSSI da primeira observacao bruta;
  - o snapshot agora usa um RSSI representativo do frame agregado;
  - a expiracao do `BssidRegistry` no modo `netsh` foi reduzida de 30s para 10s para diminuir BSSIDs stale em testes locais.
  - o modo Wi-Fi basico agora limita a contagem de pessoas em cenarios de baixa visibilidade (`1-2` observacoes vivas);
  - o `quality_verdict` do modo basico pode subir de `Deny` para `Warn` quando houver sinal real suficiente para uso pratico, sem fingir qualidade de CSI.

## Verification Conclusion
| ID | Hypothesis | Status | Evidence |
|----|------------|--------|----------|
| A | O coletor RSSI do Windows nao esta conseguindo ler amostras reais do adaptador e cai em valores sentinela/degenerados. | Parcialmente confirmada | O adaptador nao era o problema principal; o parser PT-BR fazia parte do caminho cair em `-100`. Depois do fix, o runtime passou a mostrar `-58/-59` em vez de `-100`. |
| B | O pipeline recebe amostras, mas a filtragem/deduplicacao zera a variancia e o motion score antes da classificacao. | Rejeitada como causa principal | Apos o fix, `variance`, `motion_band_power` e `spectral_power` ficaram positivos e estaveis; o pipeline nao estava zerando tudo por si so. |
| C | O adaptador/rede atual entrega dados somente do AP conectado, com atualizacao lenta ou congelada, abaixo do minimo esperado pelo algoritmo. | Parcialmente confirmada | Os logs mais recentes mostram `observations = 1` enquanto o frame ainda mantem `bssid_count = 3` por janela curta de historico/cache. O modo basico continua dependente de scans lentos e pouco ricos. |
| D | O modo `wifi` esta pegando a interface errada ou um estado stale do Windows, produzindo snapshots validos do ponto de vista tecnico, mas sem relacao com o modem/rede testados. | Inconclusiva | Houve sinais de cache/staleness, mas nao ficou provado que a interface esteja errada. O principal problema observado foi localizacao + agregacao de RSSI. |
| E | A classificacao/adaptive classifier do modo basico esta inferindo `presence` de forma otimista mesmo quando o sinal base esta invalido. | Confirmada historicamente, mitigada | Antes, havia presenca com `-100`. Agora os campos principais do snapshot deixam de carregar esse RSSI invalido, e a UI ja havia recebido gating visual para nao exagerar a confianca. |

Diagnostico atual:
- O bug do `-100 fantasma` foi resolvido em dois niveis:
  1. parser localizado do `netsh`;
  2. agregacao incorreta entre `first_rssi` e frame multi-BSSID.
- O que sobra agora e uma limitacao estrutural do modo basico no Windows:
  - scans lentos,
  - poucos pontos observaveis,
  - historico curto ainda sujeito a staleness/cache do `netsh`.
- A sessao continua `[OPEN]` ate confirmacao sua em uso real do Observatory, porque ainda precisamos validar a sensacao final na interface.
