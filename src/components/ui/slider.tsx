import { cn } from "@/lib/utils"
import * as RadixSlider from "radix-ui"

interface SliderProps {
  value: number
  min?: number
  max?: number
  step?: number
  onValueChange: (value: number) => void
  disabled?: boolean
  ariaLabel: string
  className?: string
}

function Slider({
  value,
  min = 0,
  max = 1,
  step = 0.05,
  onValueChange,
  disabled = false,
  ariaLabel,
  className,
}: SliderProps) {
  return (
    <RadixSlider.Slider.Root
      data-slot="slider"
      className={cn(
        "relative flex w-full touch-none select-none items-center",
        disabled && "opacity-50",
        className,
      )}
      value={[value]}
      min={min}
      max={max}
      step={step}
      disabled={disabled}
      onValueChange={(values) => {
        const next = values[0]
        if (typeof next === "number") onValueChange(next)
      }}
    >
      <RadixSlider.Slider.Track className="relative h-1.5 w-full grow overflow-hidden rounded-full bg-input">
        <RadixSlider.Slider.Range className="absolute h-full bg-primary" />
      </RadixSlider.Slider.Track>
      <RadixSlider.Slider.Thumb
        aria-label={ariaLabel}
        className="block size-4 rounded-full border border-primary/50 bg-background shadow transition-colors focus-visible:border-ring focus-visible:ring-ring/50 focus-visible:ring-[3px] focus-visible:outline-none"
      />
    </RadixSlider.Slider.Root>
  )
}

export { Slider }
