import { describe, it, expect, vi } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import { TelemetryConsentDialog } from './TelemetryConsentDialog'

const dragRegionMouseDown = vi.fn()

vi.mock('../hooks/useDragRegion', () => ({
  useDragRegion: () => ({ onMouseDown: dragRegionMouseDown }),
}))

describe('TelemetryConsentDialog', () => {
  const successfulChoice = () => Promise.resolve(true)

  it('renders the consent dialog', () => {
    render(<TelemetryConsentDialog onAccept={successfulChoice} onDecline={successfulChoice} />)
    expect(screen.getByText('Help improve Nabu')).toBeDefined()
    expect(screen.getByText(/anonymous crash reports/i)).toBeDefined()
  })

  it('calls onAccept when Allow button is clicked', () => {
    const onAccept = vi.fn().mockResolvedValue(true)
    render(<TelemetryConsentDialog onAccept={onAccept} onDecline={successfulChoice} />)
    fireEvent.click(screen.getByTestId('telemetry-accept'))
    expect(onAccept).toHaveBeenCalledOnce()
  })

  it('calls onDecline when No thanks button is clicked', () => {
    const onDecline = vi.fn().mockResolvedValue(true)
    render(<TelemetryConsentDialog onAccept={successfulChoice} onDecline={onDecline} />)
    fireEvent.click(screen.getByTestId('telemetry-decline'))
    expect(onDecline).toHaveBeenCalledOnce()
  })

  it('shows a details section explaining what data is shared', () => {
    render(<TelemetryConsentDialog onAccept={successfulChoice} onDecline={successfulChoice} />)
    expect(screen.getByText(/no vault content, note titles/i)).toBeDefined()
  })

  it('focuses the first action for keyboard users', () => {
    render(<TelemetryConsentDialog onAccept={successfulChoice} onDecline={successfulChoice} />)
    expect(screen.getByTestId('telemetry-decline')).toHaveFocus()
  })

  it('uses the surrounding surface as a drag region and excludes the dialog card', () => {
    render(<TelemetryConsentDialog onAccept={successfulChoice} onDecline={successfulChoice} />)

    const shell = screen.getByTestId('telemetry-consent-shell')
    fireEvent.mouseDown(shell)

    expect(dragRegionMouseDown).toHaveBeenCalledOnce()
    expect(shell.querySelector('[data-no-drag]')).not.toBeNull()
  })

  it('shows an actionable error and restores both actions when saving fails', async () => {
    const onDecline = vi.fn().mockResolvedValue(false)
    render(<TelemetryConsentDialog onAccept={successfulChoice} onDecline={onDecline} />)

    fireEvent.click(screen.getByTestId('telemetry-decline'))

    expect(await screen.findByRole('alert')).toHaveTextContent(/couldn.t save this choice/i)
    expect(screen.getByTestId('telemetry-decline')).toBeEnabled()
    expect(screen.getByTestId('telemetry-accept')).toBeEnabled()
  })
})
