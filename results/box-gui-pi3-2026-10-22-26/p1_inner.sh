set -x
export DEBIAN_FRONTEND=noninteractive
sudo -E apt-get update
sudo -E apt-get install -y --no-install-recommends xvfb tigervnc-standalone-server xfce4 xfce4-terminal dbus-x11 chromium fonts-dejavu-core fonts-noto-cjk xdotool scrot dbus-user-session python3-gi gir1.2-atspi-2.0 gir1.2-gtk-3.0 at-spi2-core gsettings-desktop-schemas libglib2.0-bin
sudo -E apt-get install -y libreoffice libreoffice-gtk3 gimp vlc python3-venv python3-dev build-essential python3-pyatspi libatspi2.0-dev wmctrl socat x11-utils curl unzip fonts-dejavu fonts-liberation libgirepository1.0-dev libcairo2-dev pkg-config ffmpeg x11-xserver-utils python3-tk xdg-user-dirs thunderbird poppler-utils git
echo APT_DONE
cd /tmp && curl -fsSL -o chrome.deb https://dl.google.com/linux/direct/google-chrome-stable_current_amd64.deb && sudo -E apt-get install -y ./chrome.deb; rm -f chrome.deb
curl -fsSL -o code.deb 'https://update.code.visualstudio.com/latest/linux-deb-x64/stable' && sudo -E apt-get install -y ./code.deb; rm -f code.deb
echo DEBS_DONE
google-chrome-stable --version; code --version | head -1; thunderbird --version
sudo npm i -g @earendil-works/pi-coding-agent@1.3.0
pi --version
printf 'start\n1440x900\n\n' | instabox-desktop
xdg-user-dirs-update
df -h / /data
echo P1_DONE
