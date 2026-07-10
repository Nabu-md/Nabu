import React from 'react'
import { Code } from 'mdast'
import { Prism as SyntaxHighlighter } from 'react-syntax-highlighter'
import { vscDarkPlus } from 'react-syntax-highlighter/dist/esm/styles/prism'

export interface CodeBlockProps {
  node: Code
}

/**
 * Renders a fenced code block with syntax highlighting via react-syntax-highlighter.
 * Falls back to plain pre/code rendering when no language is declared.
 * Requirements: 3.6
 */
export function CodeBlock({ node }: CodeBlockProps): React.JSX.Element {
  const language = node.lang ?? undefined

  return (
    <div className="code-block my-3">
      {node.lang && (
        <div className="text-xs font-mono px-3 py-0.5 rounded-t bg-[#1e1e1e] text-white/50 inline-block select-none">
          {node.lang}
        </div>
      )}
      {language ? (
        <SyntaxHighlighter
          language={language}
          style={vscDarkPlus}
          customStyle={{
            margin: 0,
            borderRadius: node.lang ? '0 4px 4px 4px' : '4px',
            fontSize: '0.875rem',
            lineHeight: '1.625'
          }}
          codeTagProps={{ style: { fontFamily: 'inherit' } }}
          PreTag="div"
        >
          {node.value}
        </SyntaxHighlighter>
      ) : (
        <pre
          className="overflow-x-auto rounded bg-[#1e1e1e] px-4 py-3 text-sm font-mono leading-relaxed text-white/90"
          style={{ margin: 0 }}
        >
          <code>{node.value}</code>
        </pre>
      )}
    </div>
  )
}
