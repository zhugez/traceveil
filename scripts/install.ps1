[CmdletBinding()]
param([string]$Version,[string]$Sha256,[string]$Archive,[string]$Prefix=(Join-Path $env:LOCALAPPDATA 'Traceveil'),[switch]$Force,[switch]$Uninstall)
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
if (-not $IsWindows) { throw 'This installer requires Windows PowerShell 7 or later.' }
$Prefix=[IO.Path]::GetFullPath($Prefix)
function Assert-SafePath([string]$Path) {
    $Current=$Path
    while ($Current) {
        if (Test-Path -LiteralPath $Current) {
            $Item=Get-Item -LiteralPath $Current -Force
            if ($Item.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Installation paths must not contain reparse points.' }
        }
        $Parent=[IO.Path]::GetDirectoryName($Current)
        if ($Parent -eq $Current) { break }
        $Current=$Parent
    }
}
function Save-BoundedStream([IO.Stream]$Stream,[string]$Path,[long]$Limit,[Threading.CancellationToken]$Cancellation=[Threading.CancellationToken]::None) {
    $File=[IO.File]::Open($Path,[IO.FileMode]::CreateNew)
    try { $Buffer=[byte[]]::new(65536);$Total=0;while (($Read=$Stream.ReadAsync($Buffer,0,$Buffer.Length,$Cancellation).GetAwaiter().GetResult()) -gt 0) { $Total+=$Read;if ($Total -gt $Limit) {throw 'File exceeds its size limit.'};$File.Write($Buffer,0,$Read) } } finally { $File.Dispose() }
}
$Binary=Join-Path $Prefix 'bin/traceveil.exe'
$Receipt=Join-Path $Prefix 'share/traceveil/install-receipt.json'
Assert-SafePath $Binary
Assert-SafePath $Receipt
$Owned=$false
if ((Test-Path -LiteralPath $Binary) -or (Test-Path -LiteralPath $Receipt)) {
    if (-not ((Test-Path -LiteralPath $Binary -PathType Leaf) -and (Test-Path -LiteralPath $Receipt -PathType Leaf))) { throw 'Preserve existing files without a complete Traceveil receipt.' }
    if ((Get-Item -LiteralPath $Receipt).Length -gt 8192 -or (Get-Item -LiteralPath $Binary).Length -gt 134217728) { throw 'Installed files exceed size limits.' }
    $Reader=[IO.File]::OpenRead($Receipt)
    try { $Bytes=[byte[]]::new(8193);$Used=0;while ($Used -lt $Bytes.Length) { $Count=$Reader.Read($Bytes,$Used,$Bytes.Length-$Used);if ($Count -eq 0) {break};$Used+=$Count };if ($Used -gt 8192) {throw 'Receipt exceeds 8 KiB.'};$Previous=[Text.Encoding]::UTF8.GetString($Bytes,0,$Used) | ConvertFrom-Json } finally { $Reader.Dispose() }
    if ($Previous.sha256 -cnotmatch '^[a-f0-9]{64}$') { throw 'Invalid installation receipt.' }
    if ((Get-FileHash -LiteralPath $Binary -Algorithm SHA256).Hash.ToLowerInvariant() -ne $Previous.sha256) { throw 'Installed binary was modified. Preserve it and review manually.' }
    $Owned=$true
}
if ($Uninstall) {
    if (-not $Owned) { throw 'No owned Traceveil installation at this prefix.' }
    Remove-Item -LiteralPath $Binary,$Receipt
    Write-Output 'Traceveil uninstalled. Configuration, agent skills and unrelated files were preserved.'
    exit 0
}
if ($Version -cnotmatch '^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$') { throw 'Pass an explicit -Version. No latest lookup occurs.' }
if ($Sha256 -cnotmatch '^[0-9a-fA-F]{64}$') { throw 'Pass an archive SHA256 from a trusted source.' }
$Stage=Join-Path ([IO.Path]::GetTempPath()) ('traceveil-'+[Guid]::NewGuid().ToString('N'))
$BinaryTemp=$null
$ReceiptTemp=$null
[IO.Directory]::CreateDirectory($Stage) | Out-Null
try {
    $Acl=Get-Acl -LiteralPath $Stage
    $Acl.SetAccessRuleProtection($true,$false)
    $Sid=[Security.Principal.WindowsIdentity]::GetCurrent().User
    $Acl.SetOwner($Sid)
    $Acl.AddAccessRule([Security.AccessControl.FileSystemAccessRule]::new($Sid,'FullControl','ContainerInherit,ObjectInherit','None','Allow'))
    Set-Acl -LiteralPath $Stage -AclObject $Acl
    if (-not $Archive) {
        $Arch=switch ([Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString()) { 'X64' {'x86_64'} 'Arm64' {'arm64'} default {throw 'Unsupported CPU. Build from source.'} }
        $Archive=Join-Path $Stage 'archive.zip'
        $Client=[Net.Http.HttpClient]::new()
        $Client.Timeout=[TimeSpan]::FromSeconds(120)
        $Deadline=[Threading.CancellationTokenSource]::new([TimeSpan]::FromSeconds(120))
        try { $Response=$Client.GetAsync("https://github.com/zhugez/traceveil/releases/download/v$Version/traceveil-$Version-windows-$Arch.zip",[Net.Http.HttpCompletionOption]::ResponseHeadersRead,$Deadline.Token).GetAwaiter().GetResult();try { $Response.EnsureSuccessStatusCode() | Out-Null;if ($Response.RequestMessage.RequestUri.Scheme -ne 'https') {throw 'HTTPS is required.'};$Download=$Response.Content.ReadAsStreamAsync().GetAwaiter().GetResult();try { Save-BoundedStream $Download $Archive 104857600 $Deadline.Token } finally { $Download.Dispose() } } finally { $Response.Dispose() } } finally { $Deadline.Dispose();$Client.Dispose() }
    }
    Assert-SafePath $Archive
    if ((Get-Item -LiteralPath $Archive).Length -gt 104857600) { throw 'Archive exceeds 100 MiB.' }
    $Verified=Join-Path $Stage 'verified.zip'
    $InputArchive=[IO.File]::OpenRead($Archive)
    try { Save-BoundedStream $InputArchive $Verified 104857600 } finally { $InputArchive.Dispose() }
    $Archive=$Verified
    if ((Get-FileHash -LiteralPath $Archive -Algorithm SHA256).Hash -ne $Sha256) { throw 'Archive checksum mismatch. Nothing was installed.' }
    $Zip=[IO.Compression.ZipFile]::OpenRead($Archive)
    try {
        $Entries=@($Zip.Entries | Where-Object FullName -CEQ "traceveil-$Version/traceveil.exe")
        if ($Entries.Count -ne 1 -or $Entries[0].Length -gt 134217728) { throw 'Archive lacks one bounded native binary.' }
        $Stream=$Entries[0].Open()
        $StageBinary=Join-Path $Stage 'traceveil.exe'
        try { Save-BoundedStream $Stream $StageBinary 134217728 } finally { $Stream.Dispose() }
    } finally { $Zip.Dispose() }
    if ((& $StageBinary --version) -ne "traceveil $Version" -or $LASTEXITCODE -ne 0) { throw 'Archive version does not match.' }
    if ($Owned -and -not $Force) { throw 'Use -Force to replace the receipted binary.' }
    Assert-SafePath $Binary
    Assert-SafePath $Receipt
    [IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName($Binary)) | Out-Null
    [IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName($Receipt)) | Out-Null
    $BinaryTemp=Join-Path ([IO.Path]::GetDirectoryName($Binary)) ('.traceveil-'+[Guid]::NewGuid().ToString('N'))
    $ReceiptTemp=Join-Path ([IO.Path]::GetDirectoryName($Receipt)) ('.receipt-'+[Guid]::NewGuid().ToString('N'))
    [IO.File]::Copy($StageBinary,$BinaryTemp,$false)
    [IO.File]::WriteAllText($ReceiptTemp,(@{version=$Version;sha256=(Get-FileHash -LiteralPath $StageBinary -Algorithm SHA256).Hash.ToLowerInvariant()} | ConvertTo-Json))
    [IO.File]::Move($BinaryTemp,$Binary,$true)
    $BinaryTemp=$null
    [IO.File]::Move($ReceiptTemp,$Receipt,$true)
    $ReceiptTemp=$null
    Write-Output "Installed Traceveil $Version. Add $Prefix/bin to PATH, then run traceveil demo."
} finally {
    if ($BinaryTemp -and (Test-Path -LiteralPath $BinaryTemp)) { Remove-Item -LiteralPath $BinaryTemp }
    if ($ReceiptTemp -and (Test-Path -LiteralPath $ReceiptTemp)) { Remove-Item -LiteralPath $ReceiptTemp }
    Remove-Item -LiteralPath $Stage -Recurse -Force
}
