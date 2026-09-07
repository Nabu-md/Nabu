import {
  EmbedTab,
  type FilePanelProps,
  useBlockNoteEditor,
  useComponentsContext,
  useDictionary,
} from '@blocknote/react'
import * as BlockNoteReact from '@blocknote/react'
import type { DefaultBlockSchema, DefaultInlineContentSchema, DefaultStyleSchema } from '@blocknote/core'
import { useCallback, useEffect, useState } from 'react'

interface NabuUploadTabProps extends FilePanelProps {
  setLoading: (loading: boolean) => void
}

type NabuEditor = ReturnType<typeof useBlockNoteEditor<
  DefaultBlockSchema,
  DefaultInlineContentSchema,
  DefaultStyleSchema
>>

interface UploadHandlerOptions extends NabuUploadTabProps {
  editor: NabuEditor
  setUploadFailed: (failed: boolean) => void
}

function useUploadHandler({ blockId, editor, setLoading, setUploadFailed }: UploadHandlerOptions) {
  return useCallback((file: File | null) => {
    if (!file || !editor.uploadFile) return

    setLoading(true)
    void editor.uploadFile(file, blockId)
      .then((uploaded) => {
        if (!editor.getBlock(blockId)) return
        const update = typeof uploaded === 'string'
          ? { props: { name: file.name, url: uploaded } }
          : uploaded
        editor.updateBlock(blockId, update)
      })
      .catch(() => {
        if (editor.getBlock(blockId)) setUploadFailed(true)
      })
      .finally(() => setLoading(false))
  }, [blockId, editor, setLoading, setUploadFailed])
}

function useUploadFailureReset(uploadFailed: boolean, setUploadFailed: (failed: boolean) => void) {
  useEffect(() => {
    if (!uploadFailed) return undefined
    const timer = window.setTimeout(() => setUploadFailed(false), 3_000)
    return () => window.clearTimeout(timer)
  }, [setUploadFailed, uploadFailed])
}

function matchingEntryValue<T>(record: Record<string, T>, key: string): T | undefined {
  return Object.entries(record).find(([candidateKey]) => candidateKey === key)?.[1]
}

function filePanelAccept(editor: NabuEditor, blockType: string): string {
  const blockSpec = matchingEntryValue(editor.schema.blockSpecs, blockType)
  const acceptTypes = blockSpec?.implementation.meta?.fileBlockAccept
  return acceptTypes?.length ? acceptTypes.join(',') : '*/*'
}

function filePanelPlaceholder(dictionary: ReturnType<typeof useDictionary>, blockType: string): string {
  const blockPlaceholder = matchingEntryValue(dictionary.file_panel.upload.file_placeholder, blockType)
  return blockPlaceholder
    ?? dictionary.file_panel.upload.file_placeholder.file
}

function NabuUploadTab({ blockId, setLoading }: NabuUploadTabProps) {
  const Components = useComponentsContext()
  const dictionary = useDictionary()
  const editor = useBlockNoteEditor()
  const block = editor.getBlock(blockId)
  const [uploadFailed, setUploadFailed] = useState(false)
  const handleFileChange = useUploadHandler({ blockId, editor, setLoading, setUploadFailed })
  useUploadFailureReset(uploadFailed, setUploadFailed)

  if (!Components || !block) return null

  return (
    <Components.FilePanel.TabPanel className="bn-tab-panel">
      <Components.FilePanel.FileInput
        className="bn-file-input"
        data-test="upload-input"
        accept={filePanelAccept(editor, block.type)}
        placeholder={filePanelPlaceholder(dictionary, block.type)}
        value={null}
        onChange={handleFileChange}
      />
      {uploadFailed && <div className="bn-error-text">{dictionary.file_panel.upload.upload_error}</div>}
    </Components.FilePanel.TabPanel>
  )
}

export function NabuFilePanel({ blockId }: FilePanelProps) {
  const Components = useComponentsContext()
  const dictionary = useDictionary()
  const editor = useBlockNoteEditor()
  const block = editor.getBlock(blockId)
  const [loading, setLoading] = useState(false)
  const uploadTabName = dictionary.file_panel.upload.title
  const tabs = [
    ...(editor.uploadFile ? [{
      name: uploadTabName,
      tabPanel: <NabuUploadTab blockId={blockId} setLoading={setLoading} />,
    }] : []),
    {
      name: dictionary.file_panel.embed.title,
      tabPanel: <EmbedTab blockId={blockId} />,
    },
  ]
  const [openTab, setOpenTab] = useState(uploadTabName)

  if (!Components || !block) return null

  return (
    <Components.FilePanel.Root
      className="bn-panel"
      defaultOpenTab={openTab}
      openTab={openTab}
      setOpenTab={setOpenTab}
      tabs={tabs}
      loading={loading}
    />
  )
}

export function NabuFilePanelController() {
  if (!('FilePanelController' in BlockNoteReact)) return null
  return <BlockNoteReact.FilePanelController filePanel={NabuFilePanel} />
}
