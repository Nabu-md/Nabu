/**
 * icons.tsx
 *
 * Minimal SVG icon components for Nabu's UI overhaul.
 * All icons are 18×18 by default — consistent, crisp, Obsidian-style.
 *
 * Usage:
 *   import { FilesIcon, SearchIcon, ... } from './icons'
 *   <FilesIcon className="..." />
 */

import React from 'react'

interface IconProps {
  className?: string
  size?: number
}

function IconBase({ children, className, size = 18 }: IconProps & { children: React.ReactNode }) {
  return (
    <svg
      xmlns="http://www.w3.org/2000/svg"
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.5}
      strokeLinecap="round"
      strokeLinejoin="round"
      className={className}
      aria-hidden="true"
    >
      {children}
    </svg>
  )
}

export function FilesIcon(props: IconProps) {
  return (
    <IconBase {...props}>
      <path d="M14.5 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7.5L14.5 2z" />
      <polyline points="14 2 14 8 20 8" />
    </IconBase>
  )
}

export function SearchIcon(props: IconProps) {
  return (
    <IconBase {...props}>
      <circle cx={11} cy={11} r={8} />
      <line x1={21} y1={21} x2={16.65} y2={16.65} />
    </IconBase>
  )
}

export function GraphIcon(props: IconProps) {
  return (
    <IconBase {...props}>
      <circle cx={12} cy={5} r={2.5} />
      <circle cx={5} cy={19} r={2.5} />
      <circle cx={19} cy={19} r={2.5} />
      <line x1={12} y1={7.5} x2={5} y2={16.5} />
      <line x1={12} y1={7.5} x2={19} y2={16.5} />
      <line x1={7.5} y1={17} x2={16.5} y2={17} />
    </IconBase>
  )
}

export function SettingsIcon(props: IconProps) {
  return (
    <IconBase {...props}>
      <circle cx={12} cy={12} r={3} />
      <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z" />
    </IconBase>
  )
}

export function StarIcon(props: IconProps) {
  return (
    <IconBase {...props}>
      <polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2" />
    </IconBase>
  )
}

export function TagIcon(props: IconProps) {
  return (
    <IconBase {...props}>
      <path d="M20.59 13.41l-7.17 7.17a2 2 0 0 1-2.83 0L2 12V2h10l8.59 8.59a2 2 0 0 1 0 2.82z" />
      <line x1={7} y1={7} x2="7.01" y2={7} />
    </IconBase>
  )
}

export function OutlineIcon(props: IconProps) {
  return (
    <IconBase {...props}>
      <line x1={8} y1={6} x2={21} y2={6} />
      <line x1={8} y1={12} x2={21} y2={12} />
      <line x1={8} y1={18} x2={21} y2={18} />
      <line x1={3} y1={6} x2="3.01" y2={6} />
      <line x1={3} y1={12} x2="3.01" y2={12} />
      <line x1={3} y1={18} x2="3.01" y2={18} />
    </IconBase>
  )
}

export function PlusIcon(props: IconProps) {
  return (
    <IconBase {...props}>
      <line x1={12} y1={5} x2={12} y2={19} />
      <line x1={5} y1={12} x2={19} y2={12} />
    </IconBase>
  )
}

export function FolderPlusIcon(props: IconProps) {
  return (
    <IconBase {...props}>
      <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" />
      <line x1={12} y1={11} x2={12} y2={17} />
      <line x1={9} y1={14} x2={15} y2={14} />
    </IconBase>
  )
}

export function CloseIcon(props: IconProps) {
  return (
    <IconBase {...props}>
      <line x1={18} y1={6} x2={6} y2={18} />
      <line x1={6} y1={6} x2={18} y2={18} />
    </IconBase>
  )
}

export function NoteIcon(props: IconProps) {
  return (
    <IconBase {...props}>
      <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7" />
      <path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z" />
    </IconBase>
  )
}

export function KeyboardIcon(props: IconProps) {
  return (
    <IconBase {...props}>
      <rect x={2} y={4} width={20} height={16} rx={2} />
      <line x1={6} y1={8} x2="6.01" y2={8} />
      <line x1={10} y1={8} x2="10.01" y2={8} />
      <line x1={14} y1={8} x2="14.01" y2={8} />
      <line x1={18} y1={8} x2="18.01" y2={8} />
      <line x1={6} y1={12} x2="6.01" y2={12} />
      <line x1={10} y1={12} x2="10.01" y2={12} />
      <line x1={14} y1={12} x2="14.01" y2={12} />
      <line x1={18} y1={12} x2="18.01" y2={12} />
      <line x1={6} y1={16} x2="6.01" y2={16} />
      <line x1={10} y1={16} x2="10.01" y2={16} />
      <line x1={14} y1={16} x2="14.01" y2={16} />
      <line x1={18} y1={16} x2="18.01" y2={16} />
    </IconBase>
  )
}

export function PaletteIcon(props: IconProps) {
  return (
    <IconBase {...props}>
      <circle cx={13.5} cy={6.5} r={0.5} fill="currentColor" />
      <circle cx={17.5} cy={10.5} r={0.5} fill="currentColor" />
      <circle cx={8.5} cy={7.5} r={0.5} fill="currentColor" />
      <circle cx={6.5} cy={12.5} r={0.5} fill="currentColor" />
      <path d="M12 2C6.5 2 2 6.5 2 12s4.5 10 10 10c.93 0 1.5-.67 1.5-1.5 0-.39-.15-.74-.39-1.01-.23-.26-.38-.61-.38-1 0-.83.67-1.5 1.5-1.5H16c3.31 0 6-2.69 6-6 0-5.5-4.5-10-10-10z" />
    </IconBase>
  )
}

export function FullscreenIcon(props: IconProps) {
  return (
    <IconBase {...props}>
      <path d="M8 3H5a2 2 0 0 0-2 2v3m18 0V5a2 2 0 0 0-2-2h-3m0 18h3a2 2 0 0 0 2-2v-3M3 16v3a2 2 0 0 0 2 2h3" />
    </IconBase>
  )
}

export function EditIcon(props: IconProps) {
  return (
    <IconBase {...props}>
      <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7" />
      <path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z" />
    </IconBase>
  )
}

export function EyeIcon(props: IconProps) {
  return (
    <IconBase {...props}>
      <path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z" />
      <circle cx={12} cy={12} r={3} />
    </IconBase>
  )
}

export function ExternalLinkIcon(props: IconProps) {
  return (
    <IconBase {...props}>
      <path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6" />
      <polyline points="15 3 21 3 21 9" />
      <line x1={10} y1={14} x2={21} y2={3} />
    </IconBase>
  )
}

export function SunIcon(props: IconProps) {
  return (
    <IconBase {...props}>
      <circle cx={12} cy={12} r={5} />
      <line x1={12} y1={1} x2={12} y2={3} />
      <line x1={12} y1={21} x2={12} y2={23} />
      <line x1={4.22} y1={4.22} x2={5.64} y2={5.64} />
      <line x1={18.36} y1={18.36} x2={19.78} y2={19.78} />
      <line x1={1} y1={12} x2={3} y2={12} />
      <line x1={21} y1={12} x2={23} y2={12} />
      <line x1={4.22} y1={19.78} x2={5.64} y2={18.36} />
      <line x1={18.36} y1={5.64} x2={19.78} y2={4.22} />
    </IconBase>
  )
}

export function MoonIcon(props: IconProps) {
  return (
    <IconBase {...props}>
      <path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z" />
    </IconBase>
  )
}

export function ChevronLeftIcon(props: IconProps) {
  return (
    <IconBase {...props}>
      <polyline points="15 18 9 12 15 6" />
    </IconBase>
  )
}

export function ChevronRightIcon(props: IconProps) {
  return (
    <IconBase {...props}>
      <polyline points="9 18 15 12 9 6" />
    </IconBase>
  )
}

export function MoreHorizontalIcon(props: IconProps) {
  return (
    <IconBase {...props}>
      <circle cx={12} cy={12} r={1} />
      <circle cx={19} cy={12} r={1} />
      <circle cx={5} cy={12} r={1} />
    </IconBase>
  )
}

export function MicIcon(props: IconProps) {
  return (
    <IconBase {...props}>
      <path d="M12 1a3 3 0 0 0-3 3v8a3 3 0 0 0 6 0V4a3 3 0 0 0-3-3z" />
      <path d="M19 10v2a7 7 0 0 1-14 0v-2" />
      <line x1={12} y1={19} x2={12} y2={23} />
      <line x1={8} y1={23} x2={16} y2={23} />
    </IconBase>
  )
}

export function SaveIcon(props: IconProps) {
  return (
    <IconBase {...props}>
      <path d="M19 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11l5 5v11a2 2 0 0 1-2 2z" />
      <polyline points="17 21 17 13 7 13 7 21" />
      <polyline points="7 3 7 8 15 8" />
    </IconBase>
  )
}

export function DownloadIcon(props: IconProps) {
  return (
    <IconBase {...props}>
      <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
      <polyline points="7 10 12 15 17 10" />
      <line x1={12} y1={15} x2={12} y2={3} />
    </IconBase>
  )
}
