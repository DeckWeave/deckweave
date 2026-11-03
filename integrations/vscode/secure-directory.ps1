param([Parameter(Mandatory = $true)][string]$Directory)
$ErrorActionPreference = 'Stop'
$absolute = [IO.Path]::GetFullPath($Directory)
$expected = Join-Path ([Environment]::GetFolderPath('LocalApplicationData')) 'DeckWeave\workspace-vscode'
if ($absolute -ne [IO.Path]::GetFullPath($expected)) { throw 'Unexpected companion credentials directory' }
[IO.Directory]::CreateDirectory($absolute) | Out-Null
$identity = [Security.Principal.WindowsIdentity]::GetCurrent().User
$security = [Security.AccessControl.DirectorySecurity]::new()
$security.SetAccessRuleProtection($true, $false)
$security.SetOwner($identity)
$rule = [Security.AccessControl.FileSystemAccessRule]::new($identity, 'FullControl', 'ContainerInherit,ObjectInherit', 'None', 'Allow')
$security.AddAccessRule($rule)
Set-Acl -LiteralPath $absolute -AclObject $security
