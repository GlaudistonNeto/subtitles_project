#!/usr/bin/env bash
# Script para tentar instalar libclang de forma alternativa

set -e

echo "=== Tenta Instalar libclang Alternativo ==="

# Verificar se clang já está instalado
if command -v clang &> /dev/null; then
    echo "✅ clang encontrado:"
    clang --version
    CLANG_VERSION=$(clang --version | grep "^clang" | awk '{print $2}')
    
    # Tentar encontrar libclang no diretório do clang
    if [ -d "/usr/lib/llvm-$CLANG_VERSION/lib" ]; then
        export LIBCLANG_PATH="/usr/lib/llvm-${CLANG_VERSION}/lib"
        echo "✅ Encontrado: $LIBCLANG_PATH"
        
        # Verificar se libclang está lá
        if [ -f "$LIBCLANG_PATH/lib/libclang.so*" ]; then
            echo "✅ libclang.so encontrado!"
            ls -la "$LIBCLANG_PATH/lib/libclang.so*"
        else
            echo "⚠️  libclang.so não encontrado neste diretório"
            echo "Procurando em outras localizações..."
            
            # Procurar em outros lugares comuns
            find /usr -name "libclang.so*" 2>/dev/null | head -5 || true
        fi
    else
        echo "⚠️  Diretório LLVM não encontrado"
        export LIBCLANG_PATH=""
    fi
else
    echo "❌ clang não instalado no sistema"
    echo ""
    echo "Opções:"
    echo "1. Instalar clang: sudo apt-get install -y llvm-dev libclang-dev"
    echo "2. Usar Docker container para compilar"
    echo "3. Baixar pre-build da release do GitHub"
fi

# Se LIBCLANG_PATH não estiver setado e tivermos root, tentar instalar
if [ -z "$LIBCLANG_PATH" ] && [ "$(id -u)" -eq 0 ]; then
    echo ""
    echo "Tentando instalar via apt-get..."
    apt-get update
    DEBIAN_FRONTEND=noninteractive apt-get install -y libclang-dev llvm-dev
    
    # Verificar instalação
    if command -v clang &> /dev/null; then
        export LIBCLANG_PATH="$(clang --version | grep "^clang" | awk '{print $2}'-|cut -d'/' -f1)/lib"
        echo "✅ libclang instalado!"
    fi
else
    # Em modo rootless, documentar o problema
    if [ "$USER" != "root" ]; then
        cat << 'EOF'

⚠️  Ambiente Rodando Rootless

Você não tem permissão para instalar pacotes de sistema.

Soluções Alternativas:

1. Usar Docker Container
   ```bash
   docker run --rm -it \
       -v "$(pwd)/rust/whisper-runtime:/build" \
       rust:latest bash \
       && cd /build \
       && apt-get update \
       && apt-get install -y libclang-dev build-essential cmake git \
       && cargo build --release \
       && cp target/release/subtitles ../bin/
   ```

2. Usar Máquina com Permissões Root
   Execute em uma VM ou máquina onde possa instalar libclang-dev:
   
   ```bash
   sudo apt-get install -y libclang-dev build-essential cmake git cargo
   cd /home/spike/subtitles_project/rust/whisper-runtime
   cargo build --release
   ```

3. Baixar Pre-compilado (se disponível)
   Verificar releases do GitHub do whisper-rs-sys para binários pré-compilados.

EOF
        echo ""
        echo "Para continuar: escolha uma opção acima e depois copie o binário compilado para:"
        echo "/home/spike/subtitles_project/bin/"
    fi
fi

echo ""
echo "=== Status Final ==="
if [ -n "$LIBCLANG_PATH" ]; then
    echo "✅ libclang disponível: $LIBCLANG_PATH"
    export LIBCLANG_PATH
else
    echo "❌ libclang não disponível"
fi
echo ""
echo "O binário Rust está pronto em:"
echo "/home/spike/subtitles_project/rust/whisper-runtime/src/bin/subtitles.rs"
echo ""
echo "Após compilar, copie para:"
echo "/home/spike/subtitles_project/bin/"
