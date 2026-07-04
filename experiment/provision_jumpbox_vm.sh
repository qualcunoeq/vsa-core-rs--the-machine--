#!/usr/bin/env bash
# ─── Jump-box VM Provisioning ─────────────────────────────────────────────
#
# Run this on the jump-box VM (192.168.100.2) after a minimal Debian install.
# Installs the tools The Machine uses to probe and attack the target.
#
# Usage:
#   chmod +x provision_jumpbox_vm.sh
#   sudo ./provision_jumpbox_vm.sh
#
# After provisioning, copy the jump_box binary and start the service:
#   scp your-machine:target/release/jump_box jumpbox@192.168.100.2:~/
#   sudo systemctl start jumpbox
#
# WARNING: This machine is an attack platform.  Keep it on the isolated
# network only.  The tools here (nmap, hydra) can be dangerous on any
# network with internet access.
# ────────────────────────────────────────────────────────────────────────────

set -euo pipefail

echo "═══════════════════════════════════════════════════════"
echo "  Jump-box VM Provisioning"
echo "  Isolated network: 192.168.100.2/24"
echo "  Tools: nmap, hydra, curl, netcat, pgrep"
echo "═══════════════════════════════════════════════════════"

# ── 1. Network configuration ─────────────────────────────────────────────
INTERFACE="eth0"
IP_ADDR="192.168.100.2"
NETMASK="255.255.255.0"
GATEWAY="192.168.100.1"

echo ""
echo "[1/4] Configuring network: $IP_ADDR/24 on $INTERFACE"

cat > /etc/network/interfaces.d/isolated <<NETEOF
auto $INTERFACE
iface $INTERFACE inet static
    address $IP_ADDR
    netmask $NETMASK
    gateway $GATEWAY
NETEOF

ifup $INTERFACE 2>/dev/null || true

# ── 2. Install tools ─────────────────────────────────────────────────────
echo ""
echo "[2/4] Installing attack tools"

apt-get update -qq
apt-get install -y -qq \
    nmap \
    hydra \
    curl \
    netcat-openbsd \
    procps \
    openssh-client \
    sshpass

# Verify tools
echo "  Tool versions:"
nmap --version | head -1
hydra --version 2>&1 | head -1 || echo "  hydra installed"
curl --version | head -1
nc -h 2>&1 | head -1 || echo "  nc installed"

# ── 3. Create jump-box user and directories ──────────────────────────────
echo ""
echo "[3/4] Creating jump-box service user and directories"

useradd -r -s /bin/false jumpbox 2>/dev/null || true
mkdir -p /etc/jumpbox
mkdir -p /var/log/jumpbox
chown jumpbox:jumpbox /var/log/jumpbox

# Create the allowlist file
cat > /etc/jumpbox/allowed_targets.txt <<ALLOWEOF
# Target VM — the only system the jump-box is allowed to touch
192.168.100.10

# Add additional targets below as the experiment expands:
# 192.168.100.0/24
ALLOWEOF

chown jumpbox:jumpbox /etc/jumpbox/allowed_targets.txt
chmod 644 /etc/jumpbox/allowed_targets.txt

echo "  Allowlist created at /etc/jumpbox/allowed_targets.txt"
echo "  Target: 192.168.100.10"

# ── 4. Install jump-box as a systemd service ─────────────────────────────
echo ""
echo "[4/4] Installing jump-box systemd service"

# The binary must be placed at /usr/local/bin/jump_box after build:
#   scp your-machine:target/release/jump_box jumpbox@192.168.100.2:~/
#   sudo cp ~jumpbox/jump_box /usr/local/bin/jump_box
#   sudo chmod 755 /usr/local/bin/jump_box

cat > /etc/systemd/system/jumpbox.service <<SERVICEEOF
[Unit]
Description=The Machine — Jump-box execution server
Documentation=https://github.com/qualcunoeq/the-machine
After=network.target

[Service]
Type=simple
User=jumpbox
Group=jumpbox

# The jump-box binary must be placed at this path
ExecStart=/usr/local/bin/jump_box \
    --bind 192.168.100.2:7878 \
    --allowlist /etc/jumpbox/allowed_targets.txt

# Logs go to syslog (journald) via stderr
StandardError=journal
StandardOutput=journal

# Security hardening
NoNewPrivileges=yes
PrivateTmp=yes
ProtectSystem=full
ProtectHome=yes
CapabilityBoundingSet=
# No capabilities — the tools (nmap, hydra) may need CAP_NET_RAW.
# If nmap scan types need raw sockets, add:
# AmbientCapabilities=CAP_NET_RAW
# But for TCP connect scans (-sT), no capabilities are needed.

# Restart on failure
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
SERVICEEOF

echo ""
echo "═══════════════════════════════════════════════════════"
echo "  Jump-box VM provisioning complete."
echo ""
echo "  NEXT STEPS (after building the binary on your host):"
echo ""
echo "  Option A — Copy binary:"
echo "    scp target/release/jump_box jumpbox@192.168.100.2:~/"
echo "    ssh jumpbox@192.168.100.2"
echo "    sudo cp ~/jump_box /usr/local/bin/jump_box"
echo "    sudo systemctl daemon-reload"
echo "    sudo systemctl enable --now jumpbox"
echo ""
echo "  Option B — Build directly on jump-box (if Rust is installed):"
echo "    git clone https://github.com/qualcunoeq/the-machine.git"
echo "    cd the-machine && cargo build --release --bin jump_box"
echo "    sudo cp target/release/jump_box /usr/local/bin/jump_box"
echo ""
echo "  Verify:"
echo "    sudo systemctl status jumpbox"
echo "    echo '{\"action_type\":\"ScanPort\",\"target\":\"192.168.100.10\",\"params\":{\"port\":\"22\"},\"timeout_secs\":5}' | nc -q 1 192.168.100.2 7878"
echo ""
echo "  TAKE A SNAPSHOT NOW:"
echo "    VBoxManage snapshot take jumpbox-vm clean_state"
echo "═══════════════════════════════════════════════════════"
