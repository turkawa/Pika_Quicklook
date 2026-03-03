# Pika_Quicklook
Macos like quick preview for Dolphin / KDE Plasma / Wayland

hi there.. i made a small rust app for dolphin. Its a small quicklook thingy.. like the one at macos.You select an image or txt file, press space and hold and a preview window pops up showing the image. Goes away when you release the space. I also added a 2 second hold if a letter pressed, incase someone wants to put a really long space at a filename  O_o. Its a vibe code for my use only..Dont expect support and DO NOT GIVE ME BUG REPORTS. The code is there..Do what you want with it. Like i said. i did this for my personal use and decided to share it..

USE IT AT YOUR OWN RISK!

I am using this on debian / KDE plasma / Wayland.
It is specifically for Dolphin
I have no idea how will it react on other setups
Again...Use it at your own risk

Files supported

Almost all common image formats. (png, Jpg, gif, etc.)
It also supports .txt, .conf, .config, etc.etc. 
It have syntax coloring.
It does not preview files bigger than set size. Default is 50mb.
It also only shows first 3500 chars of txt files.


HOW TO INSTALL

Install dependencies:
sudo apt install wl-clipboard libfontconfig1 libdbus-1-3 libssl-dev libxkbcommon-0

Run your script:
sudo bash setup_permissions.sh (then REBOOT).

Place your files:
Put pika-ql in any folder (example: home/username/apps/pika-quicklook)
and config.toml in ~/.config/pika-ql/
you can also skip the placing the config.toml file.
At first start, the app will create a config.tml at ~/.config/pika-ql/ with default values.

To change/find your keyboard:
paste this to terminal

grep -E 'Name=' /proc/bus/input/devices | cut -d'"' -f2

Then find your keyboard name in the list and change it at the config.toml

Open autostart and point it to your pika-ql

go to settings / Window rules
Set it as follows

Description: Quicklook
Window class (application) : exact match / pika-ql
window types: All Selected
Initial placement: force / centered
Keep above other windows: force / yes
Skip taskbar: force / yes
No titlebar and frame: force / yes

this should work :)

HOW TO USE

In Dolphin, select a file and HOLD the Spacebar.

Release Spacebar to close. Press 'Esc' to force close.

