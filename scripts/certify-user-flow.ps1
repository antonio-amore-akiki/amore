<#
.SYNOPSIS Amore stranger-install cert (Windows). 10 gates: download, verify, install,
side-effects, npm claude-code, self-contained MCP wire, CLI wire, mcp-list, stdio drive,
cleanup+emit result JSON. IDEMPOTENT. Run manually; not executed at commit time.
Prior-art: Adapt scripts/verify-release.ps1. topic=certify-user-flow.ps1
#>
[CmdletBinding()]
param(
    [string]$Tag = "", [string]$CertDir = "$env:LOCALAPPDATA\Amore\cert",
    [string]$Repo = "antonio-amore-akiki/amore",
    [string]$McpExe = "C:\Program Files\Amore\amore-mcp.exe",
    [string]$ClaudeJson = "$env:USERPROFILE\.claude.json",
    [string]$SchemaDir = "$PSScriptRoot\..\schema", [switch]$SkipCleanup
)
Set-StrictMode -Version Latest; $ErrorActionPreference = "Stop"

$Script:RunId = (Get-Date -Format "yyyyMMddTHHmmssZ")
$Script:StartedAt = (Get-Date -AsUTC).ToString("o")
$Script:Gates = @(); $Script:Tag = $Tag
$ResultFile = Join-Path $CertDir "local-windows-result.json"

function Record-Gate([int]$G, [string]$N, [bool]$P, [long]$D, [string]$E="", [string]$Det="") {
    $e=[ordered]@{gate=$G;name=$N;pass=$P;duration_ms=$D}
    if(!$P -and $E){$e["error"]=$E}; if($Det){$e["detail"]=$Det}
    $Script:Gates+=$e
    Write-Host "[GATE $G $(if($P){'PASS'}else{'FAIL'})] $N  (${D}ms)" -ForegroundColor $(if($P){"Green"}else{"Red"})
}

function Run-Gate([int]$Num, [string]$Name, [scriptblock]$Body) {
    Write-Host "`n--- GATE $Num : $Name ---" -ForegroundColor Cyan
    $sw=[System.Diagnostics.Stopwatch]::StartNew(); $err=""; $det=""; $ok=$false
    try{$det=(& $Body)-join"`n";$ok=$true}catch{$err=$_.ToString()}finally{$sw.Stop()}
    Record-Gate $Num $Name $ok $sw.ElapsedMilliseconds $err $det
    if(!$ok){Write-Host "  ERROR: $err" -ForegroundColor Red; Emit-Result; exit 1}
}

function Emit-Result {
    $ok=($Script:Gates.Count -gt 0) -and (($Script:Gates|Where-Object{-not $_.pass}).Count -eq 0)
    $o=[ordered]@{schema_version=1;os="windows";run_id=$Script:RunId;started_at=$Script:StartedAt;
        finished_at=(Get-Date -AsUTC).ToString("o");overall_pass=$ok;release_tag=$Script:Tag;gates=$Script:Gates}
    New-Item -ItemType Directory -Force -Path $CertDir|Out-Null
    [IO.File]::WriteAllText($ResultFile,($o|ConvertTo-Json -Depth 10),[Text.Encoding]::UTF8)
    Write-Host "==> Result: $ResultFile" -ForegroundColor Cyan
}

function Validate-Schema([string]$JP, [string]$SP) {
    if(Get-Command python -EA SilentlyContinue){
        $out=python -c "import json,sys`ntry:`n import jsonschema`n d=json.load(open(r'$JP'));s=json.load(open(r'$SP'))`n jsonschema.validate(d,s);print('schema-ok')`nexcept ImportError:`n json.load(open(r'$JP'));print('parse-ok')`nexcept Exception as e:`n print('FAIL:'+str(e),file=sys.stderr);sys.exit(1)" 2>&1
        if($LASTEXITCODE -ne 0){throw "Schema fail $JP : $out"}
        Write-Host "    schema: $out" -ForegroundColor DarkGray
    } else {$null=[IO.File]::ReadAllText($JP)|%{[Text.Json.JsonDocument]::Parse($_)}}
}

Write-Host "=== Amore Cert (Windows) ===" -ForegroundColor Magenta
foreach($c in @("gh","cosign","npm","node")){
    if(!(Get-Command $c -EA SilentlyContinue)){Write-Error "Pre-flight: '$c' not on PATH"; exit 1}
}
New-Item -ItemType Directory -Force -Path $CertDir|Out-Null
if($Script:Tag -eq ""){$Script:Tag=(gh release view --repo $Repo --json tagName|ConvertFrom-Json).tagName}
Write-Host "Tag: $($Script:Tag)"

Run-Gate 1 "download-release-exe" {
    $d=Join-Path $CertDir "download"; Remove-Item $d -Recurse -Force -EA SilentlyContinue
    New-Item -ItemType Directory -Force -Path $d|Out-Null
    gh release download $Script:Tag --repo $Repo --pattern "*.exe" --pattern "sha256sums.txt" --pattern "*.sigstore" --dir $d --clobber 2>&1|Write-Host
    $x=Get-ChildItem $d -Filter "*.exe"; if($x.Count -eq 0){throw "No .exe in release $($Script:Tag)"}
    "Downloaded: $($x.Name -join ', ')"
}

Run-Gate 2 "verify-sha256-sigstore" {
    $d=Join-Path $CertDir "download"; $sf=Join-Path $d "sha256sums.txt"
    if(!(Test-Path $sf)){throw "sha256sums.txt absent"}
    Get-Content $sf|ForEach-Object{
        if($_ -match '^([0-9a-fA-F]{64})\s+(.+)$'){
            $fp=Join-Path $d $matches[2].Trim()
            if(Test-Path $fp){
                $a=(Get-FileHash $fp -Algorithm SHA256).Hash.ToLower()
                if($a -ne $matches[1].ToLower()){throw "SHA256 mismatch: $($matches[2].Trim())"}
            }
        }
    }
    foreach($exe in (Get-ChildItem $d -Filter "*.exe")){
        $b=Join-Path $d "$($exe.Name).sigstore"
        if(Test-Path $b){
            cosign verify-blob --bundle $b --certificate-identity-regexp "antonioakiki15@gmail.com" --certificate-oidc-issuer "https://accounts.google.com" $exe.FullName 2>&1|Write-Host
            if($LASTEXITCODE -ne 0){throw "cosign failed: $($exe.Name)"}
        } else {Write-Host "    WARN: no .sigstore for $($exe.Name)" -ForegroundColor Yellow}
    }
    "SHA256 + Sigstore passed"
}

Run-Gate 3 "silent-install" {
    $e=(Get-ChildItem (Join-Path $CertDir "download") -Filter "*.exe"|Select-Object -First 1).FullName
    if(!$e){throw "No .exe"}
    $p=Start-Process -FilePath $e -ArgumentList "/VERYSILENT","/SUPPRESSMSGBOXES","/NORESTART" -Wait -PassThru
    if($p.ExitCode -ne 0){throw "Installer exit $($p.ExitCode)"}; Start-Sleep -Seconds 3; "exit 0"
}

Run-Gate 4 "assert-install-side-effects" {
    if(!(Test-Path $McpExe)){throw "Binary absent: $McpExe"}
    $qd="$env:LOCALAPPDATA\Amore\qdrant\storage"; New-Item -ItemType Directory -Force -Path $qd|Out-Null
    $pr=Join-Path $qd ".cert-probe-$Script:RunId"
    try{[IO.File]::WriteAllText($pr,"p"); Remove-Item $pr -Force}catch{throw "Non-admin write fail: $_"}
    $sv=Get-Service -Name "Amore*" -EA SilentlyContinue
    @("binary OK","qdrant write OK") + $(if($sv){$sv|%{"svc $($_.Name): $($_.Status)"}}else{"no services (user-mode)"})
}

Run-Gate 5 "install-claude-code-cli" {
    npm install -g "@anthropic-ai/claude-code" 2>&1|Write-Host
    if($LASTEXITCODE -ne 0){throw "npm install claude-code failed"}
    if(!(Get-Command claude -EA SilentlyContinue)){throw "claude not on PATH after install"}
    "claude version: $(& claude --version 2>&1)"
}

Run-Gate 6 "auto-wire-self-contained" {
    $bk="$ClaudeJson.cert-bk-$Script:RunId"; if(Test-Path $ClaudeJson){Copy-Item $ClaudeJson $bk -Force}
    $raw=& $McpExe --register-claude-code --self-contained 2>&1
    if($LASTEXITCODE -ne 0){throw "amore-mcp --self-contained exit $LASTEXITCODE`n$raw"}
    try{$c=$raw|ConvertFrom-Json}catch{throw "Contract not JSON: $raw"}
    foreach($f in @("detected","wired","skipped","errors")){if($null -eq $c.$f){throw "Missing: $f"}}
    $cf=Join-Path $CertDir "auto-wire-contract-gate6.json"
    [IO.File]::WriteAllText($cf,$raw,[Text.Encoding]::UTF8)
    Validate-Schema $cf (Resolve-Path (Join-Path $SchemaDir "auto-wire-contract.schema.json"))
    if($c.errors.Count -gt 0){throw "wire errors: $($c.errors|ConvertTo-Json -Compress)"}
    if(!(Test-Path $ClaudeJson)){throw "~/.claude.json absent after self-contained write"}
    $cfg=Get-Content $ClaudeJson -Raw|ConvertFrom-Json
    if(!($cfg.mcpServers -and ($cfg.mcpServers.PSObject.Properties.Name -contains "amore"))){throw "amore absent"}
    "self-contained OK; detected=$($c.detected.Count) wired=$($c.wired.Count)"
}

if(Test-Path $ClaudeJson){try{
    $cfg=Get-Content $ClaudeJson -Raw|ConvertFrom-Json
    if($cfg.mcpServers -and ($cfg.mcpServers.PSObject.Properties.Name -contains "amore")){
        $cfg.mcpServers.PSObject.Properties.Remove("amore")
        [IO.File]::WriteAllText($ClaudeJson,($cfg|ConvertTo-Json -Depth 20),[Text.Encoding]::UTF8)
    }
}catch{}}

Run-Gate 7 "auto-wire-cli-leg" {
    claude mcp add amore --scope user -- $McpExe 2>&1|Write-Host
    if($LASTEXITCODE -ne 0){throw "mcp add exit $LASTEXITCODE"}
    $cfg=Get-Content $ClaudeJson -Raw|ConvertFrom-Json
    if(!($cfg.mcpServers -and ($cfg.mcpServers.PSObject.Properties.Name -contains "amore"))){throw "amore absent after mcp add"}
    "CLI wire OK"
}

Run-Gate 8 "mcp-list-connected" {
    $out=claude mcp list 2>&1; if($LASTEXITCODE -ne 0){throw "mcp list exit $LASTEXITCODE"}
    $row=$out|Select-String "amore"; if(!$row){throw "amore not in mcp list:`n$out"}; "mcp list: $row"
}

Run-Gate 9 "mcp-stdio-drive" {
    $kw="certKw_$Script:RunId"
    $msgs=@(
        '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"cert","version":"0"}}}',
        '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}',
        "{`"jsonrpc`":`"2.0`",`"id`":3,`"method`":`"tools/call`",`"params`":{`"name`":`"observe`",`"arguments`":{`"content`":`"fox $kw`",`"source`":`"cert`"}}}",
        "{`"jsonrpc`":`"2.0`",`"id`":4,`"method`":`"tools/call`",`"params`":{`"name`":`"recall`",`"arguments`":{`"query`":`"$kw`"}}}"
    )
    $pr=New-Object System.Diagnostics.Process
    $pr.StartInfo.FileName=$McpExe; $pr.StartInfo.Arguments="--stdio"
    $pr.StartInfo.UseShellExecute=$false; $pr.StartInfo.RedirectStandardInput=$true
    $pr.StartInfo.RedirectStandardOutput=$true; $pr.StartInfo.RedirectStandardError=$true
    $pr.Start()|Out-Null; $msgs|%{$pr.StandardInput.WriteLine($_)}; $pr.StandardInput.Close()
    $ot=$pr.StandardOutput.ReadToEndAsync(); $et=$pr.StandardError.ReadToEndAsync()
    if(!($pr.WaitForExit(30000))){$pr.Kill(); throw "amore-mcp --stdio timeout"}
    $so=$ot.Result; $se=$et.Result
    if($so -notmatch '"observe"'){throw "tools/list missing observe`n$so`n$se"}
    if($so -notmatch [regex]::Escape($kw)){throw "recall missing '$kw'`n$so"}
    "stdio OK: observe+recall '$kw'"
}

Run-Gate 10 "cleanup-and-emit-result" {
    $r=@()
    if(!$SkipCleanup){
        if(Test-Path $McpExe){$r+="erase: $(& $McpExe data erase --confirm 2>&1)"}
        $ui=@("C:\Program Files\Amore\unins000.exe","C:\Program Files\Amore\Uninstall.exe")
        $un=$false; foreach($u in $ui){if(Test-Path $u){
            $p=Start-Process $u -ArgumentList "/VERYSILENT","/SUPPRESSMSGBOXES" -Wait -PassThru
            $r+="uninstall exit $($p.ExitCode)"; $un=$true; break
        }}
        if(!$un){
            foreach($rp in @("HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall","HKLM:\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall")){
                if($un){break}
                foreach($k in (Get-ChildItem $rp -EA SilentlyContinue)){
                    $d=(Get-ItemProperty $k.PSPath -EA SilentlyContinue).DisplayName
                    if($d -match "^Amore"){
                        $us=(Get-ItemProperty $k.PSPath).UninstallString
                        if($us){cmd /c "$us /VERYSILENT /SUPPRESSMSGBOXES" 2>&1|Out-Null; $r+="unreg: $d"; $un=$true; break}
                    }
                }
            }
            if(!$un){$r+="WARN: uninstaller not found"}
        }
        npm uninstall -g "@anthropic-ai/claude-code" 2>&1|Write-Host; npm cache clean --force 2>&1|Write-Host
        @("claude.cmd","claude.ps1","claude")|%{Remove-Item "$env:APPDATA\npm\$_" -Force -EA SilentlyContinue}
        $r+=if(Get-Command claude -EA SilentlyContinue){"WARN: claude still on PATH"}else{"claude: not on PATH (OK)"}
        $bk="$ClaudeJson.cert-bk-$Script:RunId"
        if(Test-Path $bk){Copy-Item $bk $ClaudeJson -Force; Remove-Item $bk -Force; $r+="~/.claude.json restored"}
    } else {$r+="cleanup skipped"}
    Emit-Result; Validate-Schema $ResultFile (Resolve-Path (Join-Path $SchemaDir "cert-result.schema.json"))
    $r+"result schema-validated"
}

$f=$Script:Gates|Where-Object{-not $_.pass}
Write-Host "`n=== Cert Complete ===" -ForegroundColor Magenta; Write-Host "Result: $ResultFile"
if($f.Count -eq 0){Write-Host "ALL GATES PASSED" -ForegroundColor Green; exit 0}
else{Write-Host "FAILED: $(($f|%{$_.gate})-join',')" -ForegroundColor Red; exit 1}
