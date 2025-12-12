const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

/**
 * App State
 */
const state = {
  servers: [],
  currentServerId: null,
  isRunning: false,
  isProxyEnabled: false
};

/**
 * UI Elements
 */
const ui = {
  serverSelect: document.getElementById('server-select'),
  btnAdd: document.getElementById('btn-add'),
  btnRename: document.getElementById('btn-rename'),
  btnDelete: document.getElementById('btn-delete'),
  
  inputs: {
    server: document.getElementById('server-addr'),
    listen: document.getElementById('listen-addr'),
    token: document.getElementById('token'),
    ip: document.getElementById('ip'),
    dns: document.getElementById('dns'),
    ech: document.getElementById('ech'),
    routing: document.getElementsByName('routing')
  },
  
  btnStart: document.getElementById('btn-start'),
  btnStop: document.getElementById('btn-stop'),
  btnProxy: document.getElementById('btn-proxy'),
  btnSave: document.getElementById('btn-save'),
  btnClearLog: document.getElementById('btn-clear-log'),
  logContainer: document.getElementById('log-container'),
  
  // Advanced panel toggle
  toggleAdvanced: document.getElementById('toggle-advanced'),
  advancedPanel: document.querySelector('.advanced-panel'),
  
  // Modal
  modalOverlay: document.getElementById('modal-overlay'),
  modalTitle: document.getElementById('modal-title'),
  modalInput: document.getElementById('modal-input'),
  modalConfirm: document.getElementById('modal-confirm'),
  modalCancel: document.getElementById('modal-cancel'),
  modalClose: document.getElementById('modal-close')
};

/**
 * Initialization
 */
window.addEventListener('DOMContentLoaded', async () => {
  appendLog('[系统] 初始化中...');
  
  try {
    // Determine OS specific UI tweaks
    const platform = await invoke('get_app_version'); // Just a check
    
    // Load data
    await refreshServers();
    await checkProcessStatus();
    
    // Setup listeners
    setupEventListeners();
    setupTauriListeners();
    
    appendLog('[系统] 就绪');
  } catch (err) {
    appendLog(`[错误] 初始化失败: ${err}`);
  }
});

/**
 * Event Listeners
 */
function setupEventListeners() {
  // Server Selection
  ui.serverSelect.addEventListener('change', async (e) => {
    if (state.isRunning) {
      alert("请先停止当前连接后再切换服务器");
      // Revert selection
      ui.serverSelect.value = state.currentServerId;
      return;
    }
    
    // Auto-save current before switching?
    // For now, let's just switch and load
    const newId = e.target.value;
    try {
      await invoke('set_current_server', { id: newId });
      state.currentServerId = newId;
      await loadCurrentServer();
    } catch (err) {
      appendLog(`[错误] 切换服务器失败: ${err}`);
    }
  });

  // Buttons
  ui.btnStart.addEventListener('click', startProcess);
  ui.btnStop.addEventListener('click', stopProcess);
  ui.btnProxy.addEventListener('click', toggleProxy);
  ui.btnSave.addEventListener('click', saveConfig);
  ui.btnClearLog.addEventListener('click', () => ui.logContainer.innerHTML = '');
  
  // Server Management
  ui.btnAdd.addEventListener('click', () => showModal('新增服务器', '请输入服务器名称', async (name) => {
    try {
      await invoke('add_server', { name });
      await refreshServers();
      appendLog(`[系统] 已添加服务器: ${name}`);
    } catch (err) {
      appendLog(`[错误] ${err}`);
    }
  }));
  
  ui.btnRename.addEventListener('click', () => {
    const currentName = ui.serverSelect.options[ui.serverSelect.selectedIndex].text;
    showModal('重命名服务器', '请输入新名称', async (newName) => {
      try {
        await invoke('rename_server', { id: state.currentServerId, newName });
        await refreshServers();
        appendLog(`[系统] 已重命名为: ${newName}`);
      } catch (err) {
        appendLog(`[错误] ${err}`);
      }
    }, currentName);
  });
  
  ui.btnDelete.addEventListener('click', async () => {
    if (!confirm('确定要删除当前服务器配置吗？')) return;
    try {
      await invoke('delete_server', { id: state.currentServerId });
      await refreshServers();
      appendLog(`[系统] 已删除服务器`);
    } catch (err) {
      appendLog(`[错误] ${err}`);
    }
  });

  // UI Toggles
  ui.toggleAdvanced.addEventListener('click', () => {
    ui.advancedPanel.classList.toggle('collapsed');
  });
}

/**
 * Tauri Event Listeners
 */
async function setupTauriListeners() {
  await listen('log-output', (event) => {
    appendLog(event.payload);
  });
  
  await listen('process-started', () => {
    updateProcessState(true);
    appendLog('[系统] 进程已启动');
  });
  
  await listen('process-stopped', () => {
    updateProcessState(false);
    appendLog('[系统] 进程已停止');
  });
}

/**
 * Core Functions
 */

async function refreshServers() {
  const servers = await invoke('get_servers');
  const currentId = await invoke('get_current_server_id');
  
  state.servers = servers;
  state.currentServerId = currentId;
  
  ui.serverSelect.innerHTML = '';
  servers.forEach(s => {
    const option = document.createElement('option');
    option.value = s.id;
    option.textContent = s.name;
    ui.serverSelect.appendChild(option);
  });
  
  if (currentId) {
    ui.serverSelect.value = currentId;
    await loadCurrentServer();
  }
}

async function loadCurrentServer() {
  const server = await invoke('get_current_server');
  if (!server) return;
  
  ui.inputs.server.value = server.server || '';
  ui.inputs.listen.value = server.listen || '';
  ui.inputs.token.value = server.token || '';
  ui.inputs.ip.value = server.ip || '';
  ui.inputs.dns.value = server.dns || '';
  ui.inputs.ech.value = server.ech || '';
  
  // Radio buttons
  const routing = server.routing_mode || 'bypass_cn';
  for (const radio of ui.inputs.routing) {
    if (radio.value === routing) {
      radio.checked = true;
      // Update visual style
      updateRadioVisual(radio);
    }
    // Add change listener for visuals
    radio.addEventListener('change', () => updateRadioVisual(radio));
  }
}

function updateRadioVisual(checkedRadio) {
  document.querySelectorAll('.radio-card').forEach(card => card.classList.remove('active'));
  checkedRadio.closest('.radio-card').classList.add('active');
}

async function getFormData() {
  const server = await invoke('get_current_server');
  if (!server) return null;
  
  let routingMode = 'bypass_cn';
  for (const radio of ui.inputs.routing) {
    if (radio.checked) routingMode = radio.value;
  }
  
  return {
    ...server,
    server: ui.inputs.server.value,
    listen: ui.inputs.listen.value,
    token: ui.inputs.token.value,
    ip: ui.inputs.ip.value,
    dns: ui.inputs.dns.value,
    ech: ui.inputs.ech.value,
    routing_mode: routingMode
  };
}

async function saveConfig() {
  const updatedServer = await getFormData();
  if (!updatedServer) return;
  
  try {
    await invoke('update_server', { server: updatedServer });
    appendLog('[系统] 配置已保存');
    
    // Animation feedback
    ui.btnSave.textContent = '已保存 ✓';
    setTimeout(() => {
      ui.btnSave.innerHTML = '<span class="btn-icon">💾</span><span class="btn-text">保存配置</span>';
    }, 1000);
  } catch (err) {
    appendLog(`[错误] 保存失败: ${err}`);
  }
}

async function startProcess() {
  // Save first
  await saveConfig();
  
  try {
    const msg = await invoke('start_process');
    appendLog(`[系统] ${msg}`);
    updateProcessState(true);
  } catch (err) {
    appendLog(`[错误] 启动失败: ${err}`);
  }
}

async function stopProcess() {
  try {
    // Disable proxy first if enabled
    if (state.isProxyEnabled) {
      await toggleProxy();
    }
    
    await invoke('stop_process');
    updateProcessState(false);
  } catch (err) {
    appendLog(`[错误] 停止失败: ${err}`);
  }
}

async function toggleProxy() {
  const newState = !state.isProxyEnabled;
  try {
    const msg = await invoke('set_system_proxy', { enabled: newState });
    state.isProxyEnabled = newState;
    appendLog(`[系统] ${msg}`);
    
    if (newState) {
      ui.btnProxy.classList.add('active');
      ui.btnProxy.innerHTML = '<span class="btn-icon">⚡</span><span class="btn-text">关闭系统代理</span>';
      ui.btnProxy.querySelector('.btn-icon').style.color = '#fff';
    } else {
      ui.btnProxy.classList.remove('active');
      ui.btnProxy.innerHTML = '<span class="btn-icon">⚡</span><span class="btn-text">设置系统代理</span>';
    }
  } catch (err) {
    appendLog(`[错误] 代理设置失败: ${err}`);
  }
}

async function checkProcessStatus() {
  const isRunning = await invoke('is_process_running');
  updateProcessState(isRunning);
  
  const isProxy = await invoke('get_proxy_status');
  if (isProxy) {
    state.isProxyEnabled = true;
    ui.btnProxy.innerHTML = '<span class="btn-icon">⚡</span><span class="btn-text">关闭系统代理</span>';
  }
}

function updateProcessState(isRunning) {
  state.isRunning = isRunning;
  ui.btnStart.disabled = isRunning;
  ui.btnStop.disabled = !isRunning;
  ui.btnProxy.disabled = !isRunning;
  
  // Disable inputs when running
  Object.values(ui.inputs).forEach(input => {
    if (input instanceof NodeList) {
      input.forEach(i => i.disabled = isRunning);
    } else {
      input.disabled = isRunning;
    }
  });
  ui.serverSelect.disabled = isRunning;
}

function appendLog(text) {
  const div = document.createElement('div');
  div.className = 'log-entry';
  div.textContent = text;
  
  if (text.includes('[系统]')) div.classList.add('system');
  if (text.includes('[错误]')) div.classList.add('error');
  
  ui.logContainer.appendChild(div);
  ui.logContainer.scrollTop = ui.logContainer.scrollHeight;
  
  // Limit lines
  if (ui.logContainer.children.length > 500) {
    ui.logContainer.removeChild(ui.logContainer.firstChild);
  }
}

/**
 * Modal System
 */
let modalCallback = null;

function showModal(title, placeholder, callback, defaultValue = '') {
  ui.modalTitle.textContent = title;
  ui.modalInput.placeholder = placeholder;
  ui.modalInput.value = defaultValue;
  modalCallback = callback;
  
  ui.modalOverlay.classList.add('active');
  ui.modalInput.focus();
}

function closeModal() {
  ui.modalOverlay.classList.remove('active');
  modalCallback = null;
  ui.modalInput.value = '';
}

ui.modalClose.addEventListener('click', closeModal);
ui.modalCancel.addEventListener('click', closeModal);

ui.modalConfirm.addEventListener('click', () => {
  const val = ui.modalInput.value.trim();
  if (val && modalCallback) {
    modalCallback(val);
    closeModal();
  }
});

ui.modalInput.addEventListener('keypress', (e) => {
  if (e.key === 'Enter') ui.modalConfirm.click();
});
