# Debug Session: basic-rssi-coverage
- **Status**: [CONCLUIDO — EVIDENCIA COLETADA E AJUSTE APLICADO]
- **Issue**: No modo basico Windows/RSSI (sem ESP32-S3), com varias pessoas em areas diferentes da casa (PC na mesa, cozinha, quarto), o Observatorio nao mostra sinal de cobertura multi-pessoa e o numero de pessoas nao passa de 1. Queremos confirmar se ainda estamos "deixando passar" sinal ou se isso ja e o limite real desse hardware/source.
- **Debug Server**: http://127.0.0.1:7777/event
- **Log File**: .dbg/trae-debug-log-basic-rssi-coverage.ndjson

## Pipeline encontrado (leitura estatica de codigo)

### Cap estrutural de codigo confirmado
- `practical_wifi_person_count_cap()`: havia um freio estrutural agressivo no modo `wifi:*`, principalmente quando a visibilidade caia para poucos BSSIDs.
- `score_to_person_count()`: limiares altos para adicionar pessoas:
  - 1→2 exige `smoothed_score > 0.70` (histerese down 0.55)
  - 2→3 exige `smoothed_score > 0.92`
- `practical_wifi_quality_override()`: havia um clamp pratico conservador demais no modo basico com baixa visibilidade.
- **Conclusao final da leitura + runtime**: o limite observado vinha de uma combinacao de:
  1. hardware RSSI de notebook oscilando entre fases boas (`obs_count` 4-6) e fases ruins (`obs_count` 1),
  2. heuristicas do software conservadoras demais para o caminho `wifi:*`.

### Instrumentation points aplicados
Arquivo: `v2/crates/wifi-densepose-sensing-server/src/main.rs`
| Ponto | Local | O que captura | Hipoteses |
|-------|-------|---------------|-----------|
| B | `wifi_task::after_scan` | Lista completa de BSSIDs visiveis (SSID/BSSID/RSSI/sinal/ch/banda, obs_count) | A, B, D |
| AC | `wifi_task::after_cap` | `raw_score`, `smoothed_person_score`, `pure_person_count` (antes do cap via `s.person_count()`), `cap_branch` (qual regra do practical_cap pegou), `estimated_persons_after_cap`, variance, motion, presence, quality | A, C, D |
| AC | `windows_wifi_fallback_tick::after_cap` | Mesmo do AC para o caminho fallback single-RSSI | A, C |
| E | `hybrid_poll_task` | Snapshot hybrid: `total_devices`, `fusion`, `network_devices` list com ip/mac/hostname/categoria/vendor/confidence | E |

Helpers atualizados para procurar `.dbg/` em multiplos paths relativos (WD até ../../../../).

## Status das hipoteses apos instrumentacao
| ID | Hypothesis | Likelihood | Effort | Evidence |
|----|------------|------------|--------|----------|
| A | O modo basico netsh esta lendo apenas 1 BSSID valido e o pipeline/baseline esta descartando/limiting para 1 pessoa na mesma heuristica de min_bssids | High | Low | **Parcialmente confirmada**: no inicio do teste houve `obs_count=4` e em outro trecho `obs_count=6`, mas no estado final/atual o scan caiu para `obs_count=1` (`JHON_5G`). |
| B | Netsh wlan mostra outros BSSIDs (rede vizinha, band 2.4G vs 5G) mas eles estao com RSSI qualificado como ruido estavel e nao entram na contagem | High | Low | **Parcialmente confirmada**: o scanner enxergou multiplos BSSIDs no comeco (`2.4G + 5G + radios extras`), depois colapsou para um unico BSSID visivel. |
| C | A pipeline esta capturando RSSI/variance validos, mas a heuristica `estimated_persons` ainda esta clampada para 1 no fallback basico Windows | High | Low | **Confirmada por evidencia**: `pure_person_count_via_score_to_person_count` chegou a `2` e `estimated_persons_after_cap` tambem chegou a `2` quando havia `obs_count>=4` e qualidade `Permit`. No estado final, com `obs_count=1`, o branch passou para `force_1__live_le_2`. |
| D | A qualidade do canal esta ruim e ha 3 pessoas porem o sinal real de todas elas esta abaixo do minimo medivel do RSSI bruto (pessoas longe/obstaculos pesados) | Medium | Low | **Parcialmente confirmada**: no estado final a qualidade pratica caiu para `Warn`, `variance=0`, `motion_band_power=0` e o sistema ficou cego para cobertura multipessoa. |
| E | A camada hybrid detecta varios dispositivos mas nao esta repassando limite de pessoas para a UI live do Observatory | Low | Low | **Confirmada em parte**: havia `3` dispositivos conhecidos, mas `likely_smartphones=0` em todos os eventos registrados; portanto a UI nao tinha base para ligar destaque por smartphone. |

## Findings principais
- O teste mostrou que o sistema **nao ficou preso em 1 o tempo todo**:
  - `obs_count` variou de **1** ate **6**
  - `pure_person_count_via_score_to_person_count` chegou a **2**
  - `estimated_persons_after_cap` chegou a **2**
- O estado final do print atual corresponde a uma fase pior do radio:
  - `obs_count=1`
  - `quality_verdict=Warn`
  - `variance=0`
  - `motion_band_power=0`
  - `estimated_persons=1`
- A camada hibrida detectou dispositivos na LAN, mas **nao classificou nenhum como smartphone** (`likely_smartphones=0`), entao qualquer "aura de dispositivo" percebida na UI estava ambigua visualmente e precisava de ajuste no frontend.
- O fork agora recebeu um ajuste pratico adicional no backend:
  - cap de pessoas menos travado no modo `wifi:*`,
  - quality override mais permissivo quando houver movimento real,
  - pequena retencao temporal para evitar colapso imediato de `2 -> 0/1` em oscilacoes curtas,
  - `derive_pose_from_sensing()` preservando melhor a contagem resolvida no tick.

## Reproduction Steps
1. Manter Debug Server rodando em http://127.0.0.1:7777 (ver /health OK)
2. Garantir que portas 3000/3001/8765 LIVRES
3. NDJSON limpo em `.dbg/trae-debug-log-basic-rssi-coverage.ndjson`
4. **CENARIO**: voce no PC (movendo mouse/digitando), esposa na cozinha com celular, filha no quarto com celular/aparelho ligado — manter por **3 MINUTOS**
5. **EXECUCAO**: duplo clique em `RuView\start.bat` (launcher usa `cargo run` → carrega codigo instrumentado)
6. Abrir Observatory em `http://localhost:3000/ui/observatory.html` e conferir:
   - Badge LIVE (nao DEMO)
   - Contador de pessoas na HUD (canto superior direito?)
   - Modo realista e destaque por dispositivo LIGADOS
7. Apos 3 minutos, parar a execucao (fechar janela do launcher) e coletar ndjson
8. Analisar: quantos % dos logs AC tem cap_branch force_1? Qual o max pure_person_count?

## Conclusao final
- **Nao** era correto dizer que o notebook estava extraindo "tudo" no estado anterior.
- **Sim**, havia freio de software removivel e ele foi afrouxado nesta sessao.
- **Mesmo apos o ajuste**, o modo basico continua limitado por fisica/hardware:
  - bom para **presenca geral**,
  - util para **movimento**,
  - pode sugerir **2 pessoas** em janelas boas,
  - **nao** e um caminho confiavel para separar varias pessoas em comodos diferentes com consistencia de radar/CSI.

## Proximo uso recomendado
- usar o modo basico como **sensor de presenca/movimento em tempo real**;
- usar o inventario hibrido + cadastro manual para separar melhor **pessoas x aparelhos conectados**;
- tratar Bluetooth apenas como **auxilio experimental de contexto** no Windows;
- reservar **ESP32-S3 / CSI** para o salto real de qualidade em multi-pessoa e localizacao.
