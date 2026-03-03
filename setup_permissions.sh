#!/bin/bash
# Add this to your setup_permissions.sh
echo "uinput" | sudo tee /etc/modules-load.d/uinput.conf
sudo modprobe uinput
sudo groupadd -f uinput
sudo usermod -aG uinput $USER
sudo usermod -aG input $USER
echo 'KERNEL=="uinput", GROUP="uinput", MODE="0660", OPTIONS+="static_node=uinput"' | sudo tee /etc/udev/rules.d/99-uinput.rules
echo 'KERNEL=="event*", NAME="input/%k", MODE="0660", GROUP="input"' | sudo tee /etc/udev/rules.d/99-input.rules
echo "Permissions set. REBOOT is required."
