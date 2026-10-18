#!/usr/bin/env bash
# Monta o AppImage do DataBox (Linux x86_64): um arquivo só, que roda com um duplo clique (ou `chmod +x` e
# `./DataBox-….AppImage`) na maioria das distribuições, sem instalar nada.
#
# Uso, na raiz do repositório ou de qualquer pasta:   ferramentas/appimage.sh [versão]
#   versão: o rótulo do nome do arquivo (o release.yml passa a tag, ex.: v0.1.2); sem ela, a do Cargo.toml.
# Saída: dist/DataBox-<versão>-x86_64.AppImage
#
# Compila com `cargo build --release -p editor` (nada a fazer se já compilado). O appimagetool e o runtime do
# AppImage são baixados uma vez, em versões fixas e conferidas pelo SHA-256, para ~/.cache/databox-appimage. Eles
# só montam o pacote: deles, só o runtime vai junto, no começo do arquivo, para abrir o AppImage (o do projeto
# AppImage, MIT, que leva o squashfuse e a libfuse embutidos, como todo AppImage).
#
# O que fica de fora de propósito: as bibliotecas do sistema (OpenGL/Vulkan, X11/Wayland, xkbcommon, fontes, o
# xdg-desktop-portal dos diálogos e o xdg-open). Elas vêm da distribuição, como em qualquer programa de mesa; o
# DataBox carrega o X11/Wayland e o OpenGL na hora de abrir a janela. A glibc mínima é a da máquina que compilou:
# por isso o release.yml compila no Ubuntu 22.04.
set -euo pipefail

APPIMAGETOOL_URL="https://github.com/AppImage/appimagetool/releases/download/1.9.1/appimagetool-x86_64.AppImage"
APPIMAGETOOL_SHA256="ed4ce84f0d9caff66f50bcca6ff6f35aae54ce8135408b3fa33abfc3cb384eb0"
RUNTIME_URL="https://github.com/AppImage/type2-runtime/releases/download/20251108/runtime-x86_64"
RUNTIME_SHA256="2fca8b443c92510f1483a883f60061ad09b46b978b2631c807cd873a47ec260d"

raiz="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$raiz"

versao="${1:-v$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -n 1)}"
cache="${XDG_CACHE_HOME:-$HOME/.cache}/databox-appimage"
mkdir -p "$cache"

# baixar URL SHA256 DESTINO: baixa só se não houver uma cópia com o mesmo SHA-256.
baixar() {
  local url="$1" sha="$2" destino="$3"
  if [ -f "$destino" ] && echo "$sha  $destino" | sha256sum --check --status; then
    return
  fi
  echo "Baixando $url"
  curl -fsSL --retry 3 -o "$destino.parcial" "$url"
  if ! echo "$sha  $destino.parcial" | sha256sum --check --status; then
    rm -f "$destino.parcial"
    echo "O SHA-256 de $url não confere; nada foi montado." >&2
    exit 1
  fi
  mv "$destino.parcial" "$destino"
}

appimagetool="$cache/appimagetool-1.9.1-x86_64.AppImage"
runtime="$cache/runtime-20251108-x86_64"
baixar "$APPIMAGETOOL_URL" "$APPIMAGETOOL_SHA256" "$appimagetool"
baixar "$RUNTIME_URL" "$RUNTIME_SHA256" "$runtime"
chmod +x "$appimagetool"

cargo build --release -p editor

avisos=$(ls -t target/release/build/editor-*/out/THIRD-PARTY-NOTICES.txt 2>/dev/null | head -n 1 || true)
if [ -z "$avisos" ]; then
  echo "O build não gerou o THIRD-PARTY-NOTICES.txt." >&2
  exit 1
fi

# A pasta do AppImage (AppDir): o binário, o .desktop e o ícone na raiz (o appimagetool exige), o AppRun que abre o
# binário e a licença com os avisos de terceiros.
appdir="target/appimage/DataBox.AppDir"
rm -rf "$appdir"
mkdir -p "$appdir/usr/bin" "$appdir/usr/share/applications" "$appdir/usr/share/icons/hicolor/256x256/apps" \
  "$appdir/usr/share/doc/databox"
install -m 755 target/release/DataBox "$appdir/usr/bin/DataBox"
install -m 644 crates/interface/assets/logo/databox-256.png "$appdir/usr/share/icons/hicolor/256x256/apps/databox.png"
install -m 644 crates/interface/assets/logo/databox-256.png "$appdir/databox.png"
install -m 644 LICENSE "$appdir/usr/share/doc/databox/LICENSE.txt"
install -m 644 "$avisos" "$appdir/usr/share/doc/databox/THIRD-PARTY-NOTICES.txt"

# Os mesmos tipos e categorias do atalho que o `--criar-atalho` grava (crates/editor/src/atalho.rs).
cat > "$appdir/usr/share/applications/databox.desktop" <<'FIM'
[Desktop Entry]
Type=Application
Version=1.5
Name=DataBox
Comment=Toolkit de dados gratuito e aberto para desenvolvedores, cientistas e engenheiros de dados.
Exec=DataBox %f
Icon=databox
Terminal=false
Categories=Development;Database;Utility;
MimeType=application/vnd.sqlite3;application/x-sqlite3;text/csv;text/tab-separated-values;application/json;text/plain;text/markdown;application/vnd.openxmlformats-officedocument.spreadsheetml.sheet;application/vnd.ms-excel.sheet.macroEnabled.12;
StartupWMClass=DataBox
FIM
cp "$appdir/usr/share/applications/databox.desktop" "$appdir/databox.desktop"

cat > "$appdir/AppRun" <<'FIM'
#!/bin/sh
# Abre o DataBox de dentro do AppImage com os argumentos recebidos (pasta, arquivo, --somente-leitura…).
aqui="$(dirname "$(readlink -f "$0")")"
exec "$aqui/usr/bin/DataBox" "$@"
FIM
chmod 755 "$appdir/AppRun"

mkdir -p dist
saida="dist/DataBox-$versao-x86_64.AppImage"
rm -f "$saida"
# APPIMAGE_EXTRACT_AND_RUN: o appimagetool roda sem FUSE (contêineres e o runner do CI não têm).
ARCH=x86_64 APPIMAGE_EXTRACT_AND_RUN=1 "$appimagetool" --no-appstream --runtime-file "$runtime" "$appdir" "$saida"
chmod +x "$saida"
echo "AppImage pronto: $saida"
