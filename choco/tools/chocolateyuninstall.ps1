$ErrorActionPreference = 'Stop'

$packageName = 'pick'
$installDir = "$(Split-Path -parent $MyInvocation.MyCommand.Definition)"

Remove-Item -Path (Join-Path $installDir 'pick.exe') -Force -ErrorAction SilentlyContinue
