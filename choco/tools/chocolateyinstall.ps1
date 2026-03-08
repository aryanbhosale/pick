$ErrorActionPreference = 'Stop'

$packageName = 'pick'
$version = $env:chocolateyPackageVersion
$url64 = "https://github.com/aryanbhosale/pick/releases/download/v${version}/pick-v${version}-x86_64-pc-windows-gnu.zip"

$packageArgs = @{
  packageName    = $packageName
  unzipLocation  = "$(Split-Path -parent $MyInvocation.MyCommand.Definition)"
  url64bit       = $url64
  checksum64     = $env:CHOCO_CHECKSUM
  checksumType64 = 'sha256'
}

Install-ChocolateyZipPackage @packageArgs
