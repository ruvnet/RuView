param(
    [switch]$CloseConflicts
)

$ErrorActionPreference = "Stop"
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)

$RootDir = $PSScriptRoot
$V2Dir = Join-Path $RootDir "v2"
$Timestamp = Get-Date -Format "yyyyMMdd-HHmmss"
$UiUrl = "http://127.0.0.1:3000/ui/index.html"
$UiLaunchUrl = "{0}?boot={1}" -f $UiUrl, $Timestamp
$ApiUrl = "http://127.0.0.1:3000/api/v1/sensing/latest"
$HttpPort = 3000
$WsPort = 3001
$UdpPort = 5005
$TickMs = 500
$LogDir = Join-Path $RootDir "data\launcher-logs"
$LogPath = Join-Path $LogDir "ruview-launcher-$Timestamp.log"
$script:BrowserOpened = $false
$script:LastUiProbeError = $null
$script:ServerProcess = $null
$script:ProcessJobHandle = [IntPtr]::Zero

New-Item -ItemType Directory -Force -Path $LogDir | Out-Null

if (-not ("RuViewProcessJob" -as [type])) {
    Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;

public static class RuViewProcessJob
{
    public const int JobObjectExtendedLimitInformation = 9;
    public const uint JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE = 0x00002000;

    [StructLayout(LayoutKind.Sequential)]
    public struct JOBOBJECT_BASIC_LIMIT_INFORMATION
    {
        public long PerProcessUserTimeLimit;
        public long PerJobUserTimeLimit;
        public uint LimitFlags;
        public UIntPtr MinimumWorkingSetSize;
        public UIntPtr MaximumWorkingSetSize;
        public uint ActiveProcessLimit;
        public UIntPtr Affinity;
        public uint PriorityClass;
        public uint SchedulingClass;
    }

    [StructLayout(LayoutKind.Sequential)]
    public struct IO_COUNTERS
    {
        public ulong ReadOperationCount;
        public ulong WriteOperationCount;
        public ulong OtherOperationCount;
        public ulong ReadTransferCount;
        public ulong WriteTransferCount;
        public ulong OtherTransferCount;
    }

    [StructLayout(LayoutKind.Sequential)]
    public struct JOBOBJECT_EXTENDED_LIMIT_INFORMATION
    {
        public JOBOBJECT_BASIC_LIMIT_INFORMATION BasicLimitInformation;
        public IO_COUNTERS IoInfo;
        public UIntPtr ProcessMemoryLimit;
        public UIntPtr JobMemoryLimit;
        public UIntPtr PeakProcessMemoryUsed;
        public UIntPtr PeakJobMemoryUsed;
    }

    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    public static extern IntPtr CreateJobObject(IntPtr lpJobAttributes, string lpName);

    [DllImport("kernel32.dll", SetLastError = true)]
    public static extern bool SetInformationJobObject(
        IntPtr hJob,
        int JobObjectInfoClass,
        IntPtr lpJobObjectInfo,
        uint cbJobObjectInfoLength);

    [DllImport("kernel32.dll", SetLastError = true)]
    public static extern bool AssignProcessToJobObject(IntPtr job, IntPtr process);

    [DllImport("kernel32.dll", SetLastError = true)]
    public static extern bool CloseHandle(IntPtr handle);
}
"@
}

function Write-LauncherLog {
    param(
        [Parameter(Mandatory = $true)][string]$Level,
        [Parameter(Mandatory = $true)][string]$Message
    )

    $timestamp = Get-Date -Format "HH:mm:ss"
    $line = "$timestamp [$Level] $Message"

    switch ($Level) {
        "HEADER"  { $emoji = "[RU]"; $color = "Cyan" }
        "STEP"    { $emoji = "[>>]"; $color = "Cyan" }
        "INFO"    { $emoji = "[i]"; $color = "Gray" }
        "SUCCESS" { $emoji = "[OK]"; $color = "Green" }
        "WARN"    { $emoji = "[!]"; $color = "Yellow" }
        "ERROR"   { $emoji = "[X]"; $color = "Red" }
        "SERVER"  { $emoji = "[~]"; $color = "White" }
        default   { $emoji = "[ ]"; $color = "White" }
    }

    Add-Content -Path $LogPath -Value $line
    Write-Host "$emoji $line" -ForegroundColor $color
}

function Test-RuViewUi {
    try {
        $response = Invoke-WebRequest -UseBasicParsing -Uri $UiUrl -Method Head -TimeoutSec 2
        $script:LastUiProbeError = $null
        return ($response.StatusCode -eq 200)
    }
    catch {
        $script:LastUiProbeError = $_.Exception.Message
        return $false
    }
}

function Get-BluetoothDiagnostics {
    $service = Get-Service bthserv -ErrorAction SilentlyContinue
    $adapter = Get-PnpDevice -Class Bluetooth -ErrorAction SilentlyContinue |
        Where-Object { $_.FriendlyName -and $_.FriendlyName -notmatch 'Enumerator' } |
        Select-Object -First 1

    [PSCustomObject]@{
        AdapterPresent = [bool]$adapter
        AdapterName = if ($adapter) { $adapter.FriendlyName } else { $null }
        ServiceRunning = [bool]($service -and $service.Status -eq 'Running')
        HelperMode = 'experimental'
    }
}

function Get-CargoCommandPath {
    $cargo = Get-Command cargo -ErrorAction SilentlyContinue
    if (-not $cargo) {
        throw "O comando 'cargo' nao foi encontrado no sistema."
    }
    return $cargo.Source
}

function Initialize-ProcessLifetimeGuard {
    if ($script:ProcessJobHandle -ne [IntPtr]::Zero) {
        return
    }

    $jobHandle = [RuViewProcessJob]::CreateJobObject([IntPtr]::Zero, "RuViewLauncher-$PID")
    if ($jobHandle -eq [IntPtr]::Zero) {
        throw "Nao foi possivel criar a protecao de ciclo de vida do processo."
    }

    $jobInfo = New-Object RuViewProcessJob+JOBOBJECT_EXTENDED_LIMIT_INFORMATION
    $jobInfo.BasicLimitInformation.LimitFlags = [RuViewProcessJob]::JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
    $jobInfoSize = [System.Runtime.InteropServices.Marshal]::SizeOf([type][RuViewProcessJob+JOBOBJECT_EXTENDED_LIMIT_INFORMATION])
    $jobInfoPtr = [System.Runtime.InteropServices.Marshal]::AllocHGlobal($jobInfoSize)

    try {
        [System.Runtime.InteropServices.Marshal]::StructureToPtr($jobInfo, $jobInfoPtr, $false)
        $configured = [RuViewProcessJob]::SetInformationJobObject(
            $jobHandle,
            [RuViewProcessJob]::JobObjectExtendedLimitInformation,
            $jobInfoPtr,
            [uint32]$jobInfoSize
        )

        if (-not $configured) {
            throw "Nao foi possivel configurar o encerramento automatico do processo filho."
        }
    }
    finally {
        [System.Runtime.InteropServices.Marshal]::FreeHGlobal($jobInfoPtr)
    }

    $script:ProcessJobHandle = $jobHandle
}

function Add-ProcessToLifetimeGuard {
    param(
        [Parameter(Mandatory = $true)]
        [System.Diagnostics.Process]$Process
    )

    Initialize-ProcessLifetimeGuard

    $assigned = [RuViewProcessJob]::AssignProcessToJobObject($script:ProcessJobHandle, $Process.Handle)
    if (-not $assigned) {
        throw "Nao foi possivel vincular o servidor ao ciclo de vida desta janela."
    }
}

function Stop-RuViewRuntime {
    param(
        [string]$Reason = "Encerrando instancia atual do RuView."
    )

    if ($script:ServerProcess -and -not $script:ServerProcess.HasExited) {
        Write-LauncherLog "WARN" $Reason
        try {
            & taskkill /PID $script:ServerProcess.Id /T /F | Out-Null
        }
        catch {
            try {
                Stop-Process -Id $script:ServerProcess.Id -Force -ErrorAction Stop
            }
            catch {
                Write-LauncherLog "WARN" "Nao consegui encerrar o processo principal automaticamente: $($_.Exception.Message)"
            }
        }
    }
}

function Get-RuViewRelatedProcesses {
    $currentPid = $PID
    Get-CimInstance Win32_Process | Where-Object {
        $_.ProcessId -ne $currentPid -and (
            $_.Name -in @("sensing-server.exe", "cargo.exe", "powershell.exe", "pwsh.exe") -or
            $_.CommandLine -match "wifi-densepose-sensing-server|sensing-server\.exe|start_ruview"
        )
    }
}

function Stop-ProcessTree {
    param(
        [Parameter(Mandatory = $true)]
        [int]$ProcessId
    )

    try {
        & taskkill /PID $ProcessId /T /F | Out-Null
        return $true
    }
    catch {
        try {
            Stop-Process -Id $ProcessId -Force -ErrorAction Stop
            return $true
        }
        catch {
            return $false
        }
    }
}

function Test-PortsFree {
    param(
        [int[]]$Ports
    )

    $listeners = Get-NetTCPConnection -State Listen -ErrorAction SilentlyContinue |
        Where-Object { $Ports -contains $_.LocalPort }

    return ($null -eq $listeners -or $listeners.Count -eq 0)
}

function Wait-ForPortsFree {
    param(
        [int[]]$Ports,
        [int]$TimeoutSeconds = 15
    )

    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    while ((Get-Date) -lt $deadline) {
        if (Test-PortsFree -Ports $Ports) {
            return $true
        }

        Start-Sleep -Milliseconds 400
    }

    return (Test-PortsFree -Ports $Ports)
}

function Stop-RuViewConflicts {
    param(
        [int[]]$Ports
    )

    $killed = @{}

    foreach ($port in $Ports) {
        $listeners = Get-NetTCPConnection -State Listen -LocalPort $port -ErrorAction SilentlyContinue
        foreach ($listener in $listeners) {
            $procId = $listener.OwningProcess
            if (-not $procId -or $procId -eq $PID -or $killed.ContainsKey($procId)) {
                continue
            }

            try {
                $proc = Get-CimInstance Win32_Process -Filter "ProcessId = $procId" -ErrorAction Stop
                Write-LauncherLog "WARN" "Encerrando processo na porta ${port}: PID $procId ($($proc.Name))"
                if (Stop-ProcessTree -ProcessId $procId) {
                    $killed[$procId] = $true
                    Start-Sleep -Milliseconds 400
                }
                else {
                    Write-LauncherLog "WARN" "Nao consegui encerrar a arvore do PID $procId preso na porta ${port}."
                }
            }
            catch {
                Write-LauncherLog "WARN" "Nao consegui encerrar o PID $procId preso na porta ${port}: $($_.Exception.Message)"
            }
        }
    }

    $related = Get-RuViewRelatedProcesses
    foreach ($proc in $related) {
        if ($killed.ContainsKey($proc.ProcessId)) {
            continue
        }

        $commandLine = if ($null -ne $proc.CommandLine) { $proc.CommandLine } else { "" }
        $isRuViewProcess = (
            $proc.Name -eq "sensing-server.exe" -or
            $commandLine -match "wifi-densepose-sensing-server|sensing-server\.exe|start_ruview"
        )

        if (-not $isRuViewProcess) {
            continue
        }

        try {
            Write-LauncherLog "WARN" "Encerrando instancia antiga: PID $($proc.ProcessId) ($($proc.Name))"
            if (Stop-ProcessTree -ProcessId $proc.ProcessId) {
                $killed[$proc.ProcessId] = $true
                Start-Sleep -Milliseconds 400
            }
            else {
                Write-LauncherLog "WARN" "Nao consegui encerrar a arvore do PID $($proc.ProcessId)."
            }
        }
        catch {
            Write-LauncherLog "WARN" "Nao consegui encerrar o PID $($proc.ProcessId): $($_.Exception.Message)"
        }
    }

    if ($killed.Count -gt 0) {
        Write-LauncherLog "SUCCESS" "Limpeza concluida. Processos encerrados: $($killed.Count)"
    }
    else {
        Write-LauncherLog "INFO" "Nenhum processo antigo do RuView foi encontrado."
    }

    if (-not (Wait-ForPortsFree -Ports $Ports -TimeoutSeconds 15)) {
        $blocked = Get-NetTCPConnection -State Listen -ErrorAction SilentlyContinue |
            Where-Object { $Ports -contains $_.LocalPort } |
            Select-Object -ExpandProperty LocalPort -Unique
        throw "As portas do RuView ainda estao ocupadas apos a limpeza: $($blocked -join ', ')."
    }

    Write-LauncherLog "SUCCESS" "Portas do RuView livres e prontas para uma nova inicializacao."
}

function Convert-ServerLine {
    param([string]$Line)

    $trimmed = $Line.Trim()
    if ([string]::IsNullOrWhiteSpace($trimmed)) {
        return
    }

    if ($trimmed -match "^warning:") {
        Write-LauncherLog "WARN" "Cargo: $trimmed"
        return
    }

    if ($trimmed -match "^error(\[[^\]]+\])?:") {
        Write-LauncherLog "ERROR" "Cargo: $trimmed"
        return
    }

    if ($trimmed -match "^\s*Compiling\s+(.+)$") {
        Write-LauncherLog "STEP" "Compilando componente: $($Matches[1])"
        return
    }

    if ($trimmed -match "^\s*Finished\s+.+target\(s\)\s+in\s+(.+)$") {
        Write-LauncherLog "SUCCESS" "Compilacao concluida em $($Matches[1])."
        return
    }

    if ($trimmed -match "^\s*Running\s+(.+)$") {
        Write-LauncherLog "STEP" "Executando: $($Matches[1])"
        return
    }

    if ($trimmed -like "*INFO sensing_server:*HTTP:*") {
        $httpValue = ($trimmed.Substring($trimmed.IndexOf("HTTP:") + 5)).Trim()
        Write-LauncherLog "SUCCESS" "Interface HTTP disponivel em $httpValue"
        return
    }

    if ($trimmed -like "*INFO sensing_server:*WebSocket:*") {
        $wsValue = ($trimmed.Substring($trimmed.IndexOf("WebSocket:") + 10)).Trim()
        Write-LauncherLog "INFO" "WebSocket ativo em $wsValue"
        return
    }

    if ($trimmed -like "*INFO sensing_server:*UDP:*") {
        $udpValue = ($trimmed.Substring($trimmed.IndexOf("UDP:") + 4)).Trim()
        Write-LauncherLog "INFO" "Escuta UDP preparada em $udpValue"
        return
    }

    if ($trimmed -like "*INFO sensing_server:*UI path:*") {
        $uiValue = ($trimmed.Substring($trimmed.IndexOf("UI path:") + 8)).Trim()
        Write-LauncherLog "INFO" "Pasta da interface: $uiValue"
        return
    }

    if ($trimmed -like "*INFO sensing_server:*Source:*") {
        $sourceValue = ($trimmed.Substring($trimmed.IndexOf("Source:") + 7)).Trim()
        Write-LauncherLog "INFO" "Fonte de dados selecionada: $sourceValue"
        return
    }

    if ($trimmed -like "*INFO sensing_server:*Data source:*") {
        $dataValue = ($trimmed.Substring($trimmed.IndexOf("Data source:") + 12)).Trim()
        Write-LauncherLog "INFO" "Motor de captura: $dataValue"
        return
    }

    if ($trimmed -like "*INFO sensing_server:*Vital sign detector sample rate:*") {
        $sampleValue = ($trimmed.Substring($trimmed.IndexOf("Vital sign detector sample rate:") + 32)).Trim()
        Write-LauncherLog "INFO" "Taxa do detector de sinais vitais: $sampleValue"
        return
    }

    if ($trimmed -like "*INFO sensing_server:*WiFi-DensePose Sensing Server*") {
        Write-LauncherLog "SUCCESS" "Servidor RuView iniciado."
        return
    }

    if ($trimmed -match "INFO sensing_server:\s+HTTP:\s+(.+)$") {
        Write-LauncherLog "SUCCESS" "Interface HTTP disponivel em $($Matches[1])"
        return
    }

    if ($trimmed -match "INFO sensing_server:\s+WebSocket:\s+(.+)$") {
        Write-LauncherLog "INFO" "WebSocket ativo em $($Matches[1])"
        return
    }

    if ($trimmed -match "INFO sensing_server:\s+UDP:\s+(.+)$") {
        Write-LauncherLog "INFO" "Escuta UDP preparada em $($Matches[1])"
        return
    }

    if ($trimmed -match "INFO sensing_server:\s+UI path:\s+(.+)$") {
        Write-LauncherLog "INFO" "Pasta da interface: $($Matches[1])"
        return
    }

    if ($trimmed -match "INFO sensing_server:\s+Source:\s+(.+)$") {
        Write-LauncherLog "INFO" "Fonte de dados selecionada: $($Matches[1])"
        return
    }

    if ($trimmed -match "INFO sensing_server:\s+Data source:\s+(.+)$") {
        Write-LauncherLog "INFO" "Motor de captura: $($Matches[1])"
        return
    }

    if ($trimmed -match "INFO sensing_server:\s+Vital sign detector sample rate:\s+(.+)$") {
        Write-LauncherLog "INFO" "Taxa do detector de sinais vitais: $($Matches[1])"
        return
    }

    if ($trimmed -match "INFO sensing_server:\s+Loaded runtime config:\s+(.+)$") {
        Write-LauncherLog "INFO" "Configuracao carregada: $($Matches[1])"
        return
    }

    if ($trimmed -match "INFO sensing_server:\s+Loaded adaptive classifier:\s+(.+)$") {
        Write-LauncherLog "INFO" "Classificador adaptativo carregado: $($Matches[1])"
        return
    }

    if ($trimmed -match "INFO sensing_server:\s+WiFi RSSI pipeline active\s+\((.+)\)$") {
        Write-LauncherLog "SUCCESS" "Pipeline Wi-Fi RSSI ativo ($($Matches[1]))"
        return
    }

    if ($trimmed -match "INFO sensing_server:\s+HTTP server listening on\s+(.+)$") {
        Write-LauncherLog "SUCCESS" "Servidor HTTP ouvindo em $($Matches[1])"
        return
    }

    if ($trimmed -match "INFO sensing_server:\s+WebSocket server listening on\s+(.+)$") {
        Write-LauncherLog "SUCCESS" "Servidor WebSocket ouvindo em $($Matches[1])"
        return
    }

    if ($trimmed -match "INFO sensing_server:\s+Open\s+(.+)\s+in your browser$") {
        Write-LauncherLog "INFO" "Abra no navegador: $($Matches[1])"
        return
    }

    if ($trimmed -match "INFO sensing_server:\s+API auth: OFF") {
        Write-LauncherLog "WARN" "Autenticacao da API desativada para ambiente local."
        return
    }

    if ($trimmed -match "INFO sensing_server:\s+RuView discovery skipped") {
        Write-LauncherLog "INFO" "Descoberta local ignorada porque o bind esta apenas em loopback."
        return
    }

    if ($trimmed -match "INFO wifi_densepose_sensing_server::browser_session:\s+browser session secret: loaded path=(.+)$") {
        Write-LauncherLog "INFO" "Chave da sessao do navegador carregada de: $($Matches[1])"
        return
    }

    if ($trimmed -match "INFO sensing_server:\s+Discovered\s+(\d+)\s+model files,\s+(\d+)\s+recording files$") {
        Write-LauncherLog "INFO" "Arquivos encontrados: modelos=$($Matches[1]) | gravacoes=$($Matches[2])"
        return
    }

    if ($trimmed -match "INFO sensing_server:\s+Host-header validation ON") {
        Write-LauncherLog "INFO" "Validacao de host-header ativada."
        return
    }

    if ($trimmed -match "WARN") {
        Write-LauncherLog "WARN" $trimmed
        return
    }

    if ($trimmed -match "ERROR|thread 'main' panicked|panicked at") {
        Write-LauncherLog "ERROR" $trimmed
        return
    }

    Write-LauncherLog "SERVER" $trimmed
}

function Start-RuViewBrowser {
    if (-not $script:BrowserOpened) {
        try {
            Start-Process -FilePath $UiLaunchUrl -ErrorAction Stop | Out-Null
            $script:BrowserOpened = $true
            Write-LauncherLog "SUCCESS" "Interface aberta no navegador."
        }
        catch {
            Write-LauncherLog "WARN" "Nao consegui abrir o navegador automaticamente: $($_.Exception.Message)"
            Write-LauncherLog "INFO" "Abra manualmente: $UiLaunchUrl"
        }

        Write-LauncherLog "INFO" "URL da interface: $UiLaunchUrl"
        Write-LauncherLog "INFO" "API local: $ApiUrl"
    }
}

try {
    Write-LauncherLog "HEADER" "RuView Launcher PT-BR"
    Write-LauncherLog "INFO" "Log desta sessao: $LogPath"
    Write-LauncherLog "STEP" "Validando estrutura do projeto..."

    if (-not (Test-Path (Join-Path $V2Dir "Cargo.toml"))) {
        throw "Nao encontrei a pasta 'v2' do projeto em '$V2Dir'."
    }

    $cargoPath = Get-CargoCommandPath
    Write-LauncherLog "SUCCESS" "Cargo localizado em: $cargoPath"

    if ($CloseConflicts -or $env:RUVIEW_CLOSE_CONFLICTS -eq "1") {
        Write-LauncherLog "STEP" "Encerrando instancias antigas e portas em conflito..."
        Stop-RuViewConflicts -Ports @($HttpPort, $WsPort, 8765)
    }

    if (Test-RuViewUi) {
        Write-LauncherLog "SUCCESS" "O RuView ja esta respondendo na porta $HttpPort."
        Start-RuViewBrowser
        Write-LauncherLog "INFO" "Nenhuma nova instancia foi iniciada."
        return
    }

    Write-LauncherLog "STEP" "Preparando subida do servidor..."
    Write-LauncherLog "INFO" "Modo: basico Wi-Fi (RSSI-only)"
    Write-LauncherLog "INFO" "Portas esperadas: HTTP=$HttpPort | WS=$WsPort | UDP=$UdpPort"
    Write-LauncherLog "INFO" "Intervalo de leitura: ${TickMs}ms"

    $bluetooth = Get-BluetoothDiagnostics
    if ($bluetooth.AdapterPresent) {
        Write-LauncherLog "INFO" "Bluetooth auxiliar detectado: $($bluetooth.AdapterName)"
    }
    else {
        Write-LauncherLog "WARN" "Nenhum adaptador Bluetooth utilizavel foi detectado para o modo auxiliar."
    }

    if ($bluetooth.ServiceRunning) {
        Write-LauncherLog "INFO" "Servico Bluetooth ativo. O modo auxiliar experimental pode ser consultado pela interface."
    }
    else {
        Write-LauncherLog "WARN" "Servico Bluetooth inativo. O RuView segue funcionando, mas sem o apoio auxiliar experimental."
    }

    $arguments = "run -p wifi-densepose-sensing-server -- --source wifi --http-port $HttpPort --ws-port $WsPort --tick-ms $TickMs"

    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = $cargoPath
    $psi.Arguments = $arguments
    $psi.WorkingDirectory = $V2Dir
    $psi.UseShellExecute = $false
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true
    $psi.StandardOutputEncoding = [System.Text.UTF8Encoding]::new($false)
    $psi.StandardErrorEncoding = [System.Text.UTF8Encoding]::new($false)

    $process = New-Object System.Diagnostics.Process
    $process.StartInfo = $psi
    $process.EnableRaisingEvents = $true

    $stdoutEvent = Register-ObjectEvent -InputObject $process -EventName OutputDataReceived -Action {
        if ($EventArgs.Data) {
            Convert-ServerLine -Line $EventArgs.Data
        }
    }

    $stderrEvent = Register-ObjectEvent -InputObject $process -EventName ErrorDataReceived -Action {
        if ($EventArgs.Data) {
            Convert-ServerLine -Line $EventArgs.Data
        }
    }

    Write-LauncherLog "STEP" "Iniciando processo do servidor..."
    $null = $process.Start()
    $script:ServerProcess = $process
    Add-ProcessToLifetimeGuard -Process $process
    $process.BeginOutputReadLine()
    $process.BeginErrorReadLine()

    Write-LauncherLog "SUCCESS" "Processo iniciado com PID $($process.Id)."
    Write-LauncherLog "INFO" "Aguarde alguns segundos enquanto o projeto compila/inicializa."
    Write-LauncherLog "INFO" "Esta janela permanece visivel durante toda a execucao."
    Write-LauncherLog "INFO" "Se voce fechar esta janela, o RuView sera encerrado junto."

    $deadline = (Get-Date).AddSeconds(90)
    while (-not $process.HasExited) {
        if (-not $script:BrowserOpened -and (Test-RuViewUi)) {
            Start-RuViewBrowser
        }

        if (-not $script:BrowserOpened -and (Get-Date) -ge $deadline) {
            Write-LauncherLog "WARN" "A interface ainda nao respondeu dentro de 90 segundos."
            Write-LauncherLog "WARN" "Se a compilacao ainda estiver rolando, aguarde mais um pouco."
            $deadline = (Get-Date).AddYears(10)
        }

        Start-Sleep -Seconds 2
    }

    Start-Sleep -Milliseconds 500

    if ($process.ExitCode -eq 0) {
        Write-LauncherLog "SUCCESS" "Servidor encerrado normalmente."
    }
    else {
        Write-LauncherLog "ERROR" "Servidor encerrado com falha. Codigo de saida: $($process.ExitCode)"
        if ($script:LastUiProbeError) {
            Write-LauncherLog "WARN" "Ultimo erro ao testar a interface: $script:LastUiProbeError"
        }
    }
}
catch {
    Write-LauncherLog "ERROR" $_.Exception.Message
}
finally {
    Stop-RuViewRuntime -Reason "Fechando launcher: encerrando o RuView junto com esta janela."

    Get-EventSubscriber | Where-Object {
        $_.SourceObject -is [System.Diagnostics.Process]
    } | Unregister-Event -Force -ErrorAction SilentlyContinue

    if ($script:ProcessJobHandle -ne [IntPtr]::Zero) {
        [RuViewProcessJob]::CloseHandle($script:ProcessJobHandle) | Out-Null
        $script:ProcessJobHandle = [IntPtr]::Zero
    }

    Write-LauncherLog "INFO" "Janela pronta para diagnostico. Feche quando quiser."
}
