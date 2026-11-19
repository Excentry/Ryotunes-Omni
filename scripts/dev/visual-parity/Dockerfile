FROM archlinux:latest

RUN pacman -Syu --noconfirm --needed \
      base-devel git rust clang pkgconf curl unzip \
      mpv ffmpeg yt-dlp openssl librsvg gtk3 webkit2gtk-4.1 libappindicator-gtk3 \
      quickshell qt6-declarative qt6-svg qt6-shadertools qt6-5compat qt6-multimedia \
      weston mesa pulseaudio python python-pillow socat \
      ttf-dejavu noto-fonts noto-fonts-cjk ttf-space-mono-nerd \
    && pacman -Scc --noconfirm

RUN set -eu; t=$(mktemp -d); cd "$t"; \
    curl -fsSL -o sg.zip https://github.com/floriankarsten/space-grotesk/releases/download/2.0.0/SpaceGrotesk-2.0.0.zip; \
    echo "53b415577d4139248555300710bea0d268c7a5be67b93de53b716a9736cabffd  sg.zip" | sha256sum -c -; \
    curl -fsSL -o fr.zip https://github.com/undercasetype/Fraunces/releases/download/1.000/UnderCaseType_Fraunces_1.000.zip; \
    echo "8d8b81dfaeb89433f5c908e1d8d0a4b202bd627bd80d4cd5ff56f311fdcad19f  fr.zip" | sha256sum -c -; \
    unzip -q sg.zip -d sg; unzip -q fr.zip -d fr; \
    mkdir -p /usr/share/fonts/ryoku; \
    find sg -iname '*.otf' -exec cp {} /usr/share/fonts/ryoku/ \; ; \
    cp "fr/UnderCaseType_Fraunces_1.000/Fonts - Desktop/Fraunces[SOFT,WONK,opsz,wght].ttf" /usr/share/fonts/ryoku/Fraunces-Variable.ttf; \
    fc-cache -f; rm -rf "$t"

RUN useradd -m -u 1000 ryo && install -d -o ryo -g ryo -m 700 /run/user/1000
USER ryo
ENV XDG_RUNTIME_DIR=/run/user/1000 HOME=/home/ryo CARGO_TARGET_DIR=/home/ryo/target
WORKDIR /home/ryo

RUN git clone --depth 1 https://github.com/Ryoku-dev/ryoku.git /home/ryo/ryoku \
    && bash /home/ryo/ryoku/ryoku/ui/install.sh

COPY --chown=ryo:ryo lab.sh capture.sh rpc.py /home/ryo/
RUN sed -i 's/\r$//' /home/ryo/lab.sh /home/ryo/capture.sh /home/ryo/rpc.py && chmod 755 /home/ryo/lab.sh /home/ryo/capture.sh
