// Popup script for Nabu Capture extension

document.addEventListener('DOMContentLoaded', function() {
  const capturePageBtn = document.getElementById('capture-page');
  const captureSelectionBtn = document.getElementById('capture-selection');
  const captureFullBtn = document.getElementById('capture-full');
  const statusEl = document.getElementById('status');
  const resultEl = document.getElementById('result');
  const resultContent = document.getElementById('result-content');
  const errorEl = document.getElementById('error');
  const errorContent = document.getElementById('error-content');

  function setStatus(text, type = '') {
    statusEl.textContent = text;
    statusEl.className = 'status' + (type ? ' ' + type : '');
  }

  function showResult(message) {
    resultContent.textContent = message;
    resultEl.classList.remove('hidden');
    errorEl.classList.add('hidden');
  }

  function showError(message) {
    errorContent.textContent = message;
    errorEl.classList.remove('hidden');
    resultEl.classList.add('hidden');
  }

  function hideMessages() {
    resultEl.classList.add('hidden');
    errorEl.classList.add('hidden');
  }

  function setButtonsEnabled(enabled) {
    capturePageBtn.disabled = !enabled;
    captureSelectionBtn.disabled = !enabled;
    captureFullBtn.disabled = !enabled;
  }

  async function capture(captureType, payload = {}) {
    hideMessages();
    setStatus('Capturing...', 'loading');
    setButtonsEnabled(false);

    try {
      // Get current tab
      const [tab] = await browser.tabs.query({ active: true, currentWindow: true });
      
      if (!tab) {
        throw new Error('No active tab found');
      }

      // Send message to content script
      const response = await browser.tabs.sendMessage(tab.id, {
        type: captureType === 'page' ? 'CAPTURE_PAGE' : 'CAPTURE_SELECTION'
      });

      if (!response.success) {
        throw new Error(response.error || 'Capture failed');
      }

      const captureData = response.data;

      // Send to native host via background script
      const nativeResponse = await browser.runtime.sendMessage({
        command: 'capture',
        captureType: captureType,
        payload: {
          ...captureData,
          ...payload
        }
      });

      if (nativeResponse.success) {
        const result = nativeResponse.data;
        if (result.success) {
          setStatus('Captured!', '');
          showResult(`Successfully captured: ${captureData.title || captureData.url}`);
        } else {
          setStatus('Failed', 'error');
          showError(result.error || 'Capture failed');
        }
      } else {
        setStatus('Error', 'error');
        showError(nativeResponse.error || 'Communication error');
      }
    } catch (error) {
      setStatus('Error', 'error');
      showError(error.message || 'An error occurred');
    } finally {
      setButtonsEnabled(true);
    }
  }

  capturePageBtn.addEventListener('click', () => {
    capture('bookmark');
  });

  captureSelectionBtn.addEventListener('click', () => {
    capture('note');
  });

  captureFullBtn.addEventListener('click', () => {
    capture('document');
  });
});
