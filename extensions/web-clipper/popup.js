// Simple HTML to markdown converter
function htmlToMarkdown(html) {
  const doc = new DOMParser().parseFromString(html, 'text/html');
  
  function processNode(node) {
    if (node.nodeType === Node.TEXT_NODE) {
      return node.textContent || '';
    }
    if (node.nodeType === Node.ELEMENT_NODE) {
      const el = node;
      const tag = el.tagName.toLowerCase();
      const text = Array.from(el.childNodes).map(processNode).join('');
      
      switch (tag) {
        case 'h1': return `\n# ${text}\n`;
        case 'h2': return `\n## ${text}\n`;
        case 'h3': return `\n### ${text}\n`;
        case 'p': return `${text}\n\n`;
        case 'strong':
        case 'b': return `**${text}**`;
        case 'em':
        case 'i': return `*${text}*`;
        case 'code': return '`' + text + '`';
        case 'a': return `[${text}](${el.href})`;
        case 'img': return `![${el.alt}](${el.src})`;
        case 'ul':
        case 'ol':
        case 'li': return '';
        case 'blockquote': return `\n> ${text}\n`;
        default: return text;
      }
    }
    return '';
  }
  
  return doc.body ? processNode(doc.body) : '';
}

document.getElementById('clipBtn').addEventListener('click', async () => {
  const [tab] = await chrome.tabs.query({ active: true, currentWindow: true });
  
  chrome.tabs.sendMessage(tab.id, { action: 'clip' }, async (response) => {
    if (response?.markdown) {
      const url = new URL('nabu://clip');
      url.searchParams.set('title', tab.title || 'Untitled');
      url.searchParams.set('url', tab.url || '');
      url.searchParams.set('content', response.markdown.substring(0, 10000)); // Limit content size
      
      // Try to open the custom protocol
      try {
        await chrome.tabs.create({ url: url.toString() });
      } catch {
        // Fallback: copy to clipboard
        await navigator.clipboard.writeText(`# ${tab.title}\n\n${response.markdown}\n\nSource: ${tab.url}`);
        alert('Markdown copied to clipboard. Open Nabu and paste (⌘V).');
      }
    }
  });
});