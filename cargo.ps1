# Roda o cargo com a toolchain do projeto (Rust GNU + MinGW-w64 da WinLibs).
# Uso, da raiz do projeto:  .\cargo.ps1 build --release   |   .\cargo.ps1 test -p banco
# O PowerShell engole um -- solto: passe-o entre aspas, como em  .\cargo.ps1 clippy '--' -D warnings
# O MinGW vem de DATABOX_MINGW (a pasta bin) ou, sem ela, de %LOCALAPPDATA%\Programs\winlibs-gcc16\mingw64\bin.
#
# O gcc da WinLibs lê caminhos em UTF-8 e o ld, o dlltool e o windres na página ANSI; com "í" ou "ç" no
# caminho (C:\Users\Conceição) eles se perdem. O intermediário ferramentas\vincular.rs troca esses
# caminhos pelo nome curto 8.3 antes de chamar o gcc (como linker) e as ferramentas (cópias com o nome delas,
# à frente no PATH). Ele é compilado aqui na primeira vez e quando o fonte muda.
$mingw = if ($env:DATABOX_MINGW) { $env:DATABOX_MINGW } else { "$env:LOCALAPPDATA\Programs\winlibs-gcc16\mingw64\bin" }
if (-not (Test-Path -LiteralPath (Join-Path $mingw 'gcc.exe'))) {
    Write-Error "MinGW não encontrado em $mingw. Instale a WinLibs (GCC, MSVCRT) ali ou aponte DATABOX_MINGW para a pasta bin dela (veja o README)."
    exit 1
}
$bin = Join-Path $PSScriptRoot 'ferramentas\bin'
$env:PATH = "$bin;$env:USERPROFILE\.cargo\bin;$mingw;" + $env:PATH

$fonte = Join-Path $PSScriptRoot 'ferramentas\vincular.rs'
$vincular = Join-Path $bin 'vincular.exe'
$copias = @($vincular, (Join-Path $bin 'dlltool.exe'), (Join-Path $bin 'windres.exe'))
function Desatualizado {
    ($copias | Where-Object { -not (Test-Path -LiteralPath $_) }) -or
        (Get-Item -LiteralPath $fonte).LastWriteTime -gt (Get-Item -LiteralPath $vincular).LastWriteTime
}
if (Desatualizado) {
    # Vários builds podem chegar aqui juntos: um de cada vez, e quem chega depois confere de novo.
    $trava = New-Object System.Threading.Mutex($false, 'Local\DataBox-vincular')
    [void]$trava.WaitOne()
    try {
        if (Desatualizado) {
            New-Item -ItemType Directory -Force $bin | Out-Null
            $tmp = Join-Path $bin "tmp-$PID"
            New-Item -ItemType Directory -Force $tmp | Out-Null
            & rustc --edition 2024 -O $fonte -o (Join-Path $tmp 'vincular.exe')
            if ($LASTEXITCODE) { exit $LASTEXITCODE }
            # Um .exe em uso por outro build pode ser renomeado, mas não sobrescrito.
            foreach ($alvo in $copias) {
                if (Test-Path -LiteralPath $alvo) {
                    Move-Item -LiteralPath $alvo -Destination "$alvo.$PID.velho" -Force -ErrorAction SilentlyContinue
                }
                Copy-Item -LiteralPath (Join-Path $tmp 'vincular.exe') -Destination $alvo -Force
            }
            Remove-Item -LiteralPath $tmp -Recurse -Force -ErrorAction SilentlyContinue
        }
    } finally {
        $trava.ReleaseMutex()
    }
    Get-ChildItem -LiteralPath $bin -Filter '*.velho' | Remove-Item -Force -ErrorAction SilentlyContinue
    Get-ChildItem -LiteralPath $bin -Filter 'vincular.*.exe' | Remove-Item -Force -ErrorAction SilentlyContinue
}
$env:CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER = $vincular
$env:VINCULAR_DLLTOOL = Join-Path $mingw 'dlltool.exe'
$env:VINCULAR_WINDRES = Join-Path $mingw 'windres.exe'

Set-Location -LiteralPath $PSScriptRoot
& cargo @args
exit $LASTEXITCODE
