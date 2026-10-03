# Comparativo: RuView original (`HEAD`) vs nossa adaptacao BR

## 1. Baseline usado

Este comparativo usa como **projeto original** o estado limpo do repositório `RuView` em `HEAD` no branch `main`, porque este clone local não possui remoto `upstream` separado.
Ou seja:

- **Original** = `HEAD` limpo do repo
- **Adaptado / nosso** = working tree local atual + arquivos novos criados na contribuição

## 2. Visao executiva

Nossa adaptacao transforma o RuView original em uma variante mais voltada para:

- **usuarios brasileiros leigos**
- **instalacao local em Windows**
- **uso com Wi-Fi caseiro / notebook no modo basico**
- **inventario hibrido de dispositivos da rede**
- **interface traduzida e mais explicita**
- **launcher em PT-BR com logs operacionais**

Em troca, esta adaptacao assume conscientemente um escopo mais pragmatico:

- nao promete precisao de radar/CSI quando o hardware e apenas notebook + RSSI
- trata Bluetooth como **auxilio experimental**
- usa camadas hibridas e overrides manuais para separar melhor **pessoas x dispositivos**

## 3. Diferencas principais por area

### A. Inicializacao e experiencia para leigos

**Original**
- inicializacao mais tecnica
- fluxo menos orientado a usuario final local Windows
- sem launcher PT-BR dedicado no nosso workspace local

**Adaptado**
- adicionados:
  - `start.bat`
  - `start_ruview.ps1`
- o launcher:
  - limpa conflitos de porta/processo
  - abre a interface automaticamente
  - mostra logs em portugues
  - mantem o console visivel
  - agora tambem informa o estado do **Bluetooth auxiliar experimental**

### B. Traducao e localizacao PT-BR

**Original**
- UI orientada ao ingles, com cobertura local limitada para nosso publico alvo

**Adaptado**
- ampliada traducao em:
  - `ui/index.html`
  - `ui/components/DashboardTab.js`
  - `ui/components/HardwareTab.js`
  - `ui/components/LiveDemoTab.js`
  - `ui/components/SensingTab.js`
  - `ui/pose-fusion.html`
  - `ui/pose-fusion/js/main.js`
  - `ui/observatory.html`
  - `ui/utils/i18n.js`
- persistencia de idioma e textos adicionais para o fluxo BR/leigo

### C. Protecao contra cache e coerencia local

**Original**
- sem a camada local especifica que resolvemos aqui para desenvolvimento e cache do navegador no nosso ambiente

**Adaptado**
- novo arquivo:
  - `ui/utils/local-dev-cache-guard.js`
- uso de `boot=` / cache-busting e origem canonica `localhost`
- mitigacao dos problemas reais que apareciam no Chrome com scripts antigos e divergencia `127.0.0.1` vs `localhost`

### D. Modo basico Wi-Fi caseiro

**Original**
- o caminho de sensing basico estava mais conservador
- no nosso uso real, isso derrubava demais a leitura multipessoa e fazia o sistema colapsar facil para `0/1`

**Adaptado**
- ajustado:
  - `v2/crates/wifi-densepose-wifiscan/src/adapter/netsh_scanner.rs`
  - `v2/crates/wifi-densepose-sensing-server/src/main.rs`
- ganhos aplicados:
  - parser `netsh` mais robusto para contexto PT-BR
  - melhor aproveitamento do RSSI no modo `wifi:*`
  - heuristica pratica menos travada
  - pequena retencao temporal para evitar queda imediata em oscilacoes curtas
  - preservacao melhor da contagem resolvida antes de montar a pose da UI

### E. Camada hibrida de dispositivos de rede

**Original**
- nao havia no baseline limpo do repo local esta camada domestica orientada a inventario de aparelhos da casa

**Adaptado**
- novo backend:
  - `v2/crates/wifi-densepose-sensing-server/src/hybrid_detection.rs`
- integracao no servidor:
  - `v2/crates/wifi-densepose-sensing-server/src/main.rs`
- capacidade adicionada:
  - varrer dispositivos da LAN
  - inferir categoria heuristica
  - contar provaveis smartphones / smart TVs / computadores / IoT
  - combinar isso com presenca humana inferida pelo Wi-Fi

### F. Cadastro manual de dispositivos conhecidos

**Original**
- sem area dedicada para o usuario corrigir nome/categoria de aparelhos detectados

**Adaptado**
- Dashboard ganhou area para:
  - ver dispositivos detectados
  - renomear
  - corrigir categoria
  - cadastrar manualmente
  - salvar override persistente
- arquivos centrais:
  - `ui/index.html`
  - `ui/components/DashboardTab.js`
  - `ui/style.css`
  - `ui/utils/i18n.js`
  - `v2/crates/wifi-densepose-sensing-server/src/main.rs`
- novos endpoints:
  - `GET /api/v1/hybrid/overrides`
  - `POST /api/v1/hybrid/overrides/upsert`
  - `DELETE /api/v1/hybrid/overrides/delete`

### G. Bluetooth auxiliar experimental

**Original**
- sem trilha pronta de Bluetooth integrada para o nosso caso de uso

**Adaptado**
- suporte **experimental e Windows-only**
- sem vender Bluetooth como detector humano principal
- usado como contexto operacional / indicio auxiliar
- pontos alterados:
  - `start_ruview.ps1`
  - `ui/index.html`
  - `ui/components/DashboardTab.js`
  - `ui/utils/i18n.js`
  - `v2/crates/wifi-densepose-sensing-server/src/main.rs`
- novos endpoints:
  - `GET /api/v1/bluetooth/status`
  - `POST /api/v1/bluetooth/enabled`

### H. Observatory mais honesto para leigo

**Original**
- leitura visual mais ambigua no nosso uso pratico
- aura e destaque poderiam ser interpretados como algo mais preciso do que realmente eram

**Adaptado**
- alterados:
  - `ui/observatory.html`
  - `ui/observatory/css/observatory.css`
  - `ui/observatory/js/main.js`
  - `ui/observatory/js/hud-controller.js`
  - `ui/observatory/js/figure-pool.js`
  - `ui/utils/i18n.js`
- melhorias:
  - modo realista
  - destaque por dispositivo condicionado ao contexto hibrido
  - explicacao textual do significado da aura
  - bloco visivel de fusao **Wi-Fi + Bluetooth auxiliar**
  - visual mais discreto e menos enganoso

### I. README e narrativa do fork

**Original**
- README sem o recorte completo da nossa adaptacao BR/local-caseira

**Adaptado**
- `README.md` atualizado para registrar:
  - camada hibrida
  - overrides manuais
  - Bluetooth auxiliar experimental
  - honestidade sobre os limites do notebook + RSSI

## 4. Arquivos alterados no working tree

### Arquivos rastreados modificados

- `README.md`
- `ui/app.js`
- `ui/components/DashboardTab.js`
- `ui/components/HardwareTab.js`
- `ui/components/LiveDemoTab.js`
- `ui/components/SensingTab.js`
- `ui/index.html`
- `ui/observatory.html`
- `ui/observatory/css/observatory.css`
- `ui/observatory/js/figure-pool.js`
- `ui/observatory/js/hud-controller.js`
- `ui/observatory/js/main.js`
- `ui/pose-fusion.html`
- `ui/pose-fusion/js/main.js`
- `ui/style.css`
- `ui/utils/i18n.js`
- `v2/crates/wifi-densepose-sensing-server/src/main.rs`
- `v2/crates/wifi-densepose-wifiscan/src/adapter/netsh_scanner.rs`

### Arquivos novos locais relevantes da contribuicao

- `start.bat`
- `start_ruview.ps1`
- `ui/utils/local-dev-cache-guard.js`
- `v2/crates/wifi-densepose-sensing-server/src/hybrid_detection.rs`
- `docs/validation/comparativo-ruview-original-vs-adaptado-ptbr.md`
- `docs/validation/debug-basic-rssi-coverage-ptbr.md`
- `docs/validation/debug-basic-rssi-invalid-ptbr.md`

## 5. Diferenca de filosofia entre original e adaptado

### Original

- mais proximo da base tecnica/generica do projeto
- maior foco em pipeline e pesquisa
- menos opinativo para o usuario final domestico brasileiro

### Adaptado

- assume explicitamente o papel de **versao amigavel para BR**
- prioriza:
  - instalacao simples
  - feedback visual claro
  - textos em portugues
  - controle operacional local
  - honestidade sobre o que e deteccao humana e o que e apenas contexto de rede

## 6. Limites que continuam existindo

Mesmo com a adaptacao, alguns limites continuam reais:

- notebook + RSSI basico **nao vira CSI**
- multipessoa em comodos separados continua instavel
- Bluetooth entra apenas como apoio experimental
- classificacao automatica de dispositivos ainda e heuristica
- para salto real de qualidade em multi-pessoa/localizacao, o caminho continua sendo **ESP32-S3 / CSI**

## 7. Valor da contribuicao

Esta contribuicao entrega algo que o baseline original nao estava focando diretamente no nosso contexto:

- uma **versao mais utilizavel por brasileiros leigos**
- com **launcher operacional**
- **traducao mais ampla**
- **inventario hibrido**
- **cadastro manual de dispositivos**
- **Observatory mais explicativo**
- e uma abordagem mais honesta para o uso de **Wi-Fi caseiro como sensor de presenca**

Em resumo:

> O projeto original continua sendo a base tecnica.
> A nossa adaptacao transforma essa base em uma versao mais acessivel, mais local, mais transparente e mais util para uso domestico no Brasil.
