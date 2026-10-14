/** Segmented controls: a thin pill of buttons side by side, the chosen ones filled in the accent's
 *  soft tint. `SegmentedToggles` is several independent switches (the status bar's OSNAP, ORTHO,
 *  POLAR and GRID: Base UI's ToggleGroup, each a button with `aria-pressed`); `SegmentedChoice` is
 *  one choice among a few (Base UI's RadioGroup with button radios: a dialog's mode, the conflicts
 *  filter), the arrows moving the choice. Both follow the text direction. */
import { Radio } from "@base-ui/react/radio";
import { RadioGroup } from "@base-ui/react/radio-group";
import { Toggle } from "@base-ui/react/toggle";
import { ToggleGroup } from "@base-ui/react/toggle-group";
import type { ReactNode } from "react";

export type Segment<V extends string> = {
  value: V;
  label: ReactNode;
  /** The browser tooltip. */
  title?: string;
  disabled?: boolean;
};

type TogglesProps<V extends string> = {
  segments: Segment<V>[];
  /** The values that are on. */
  on: V[];
  /** The value whose switch was pressed. */
  onToggle(value: V): void;
  /** Names the group. */
  label: string;
  className?: string;
};

export function SegmentedToggles<V extends string>({ segments, on, onToggle, label, className = "" }: TogglesProps<V>) {
  return (
    <ToggleGroup
      multiple
      value={on}
      aria-label={label}
      className={`segmented ${className}`.trim()}
      onValueChange={(next) => {
        // The group hands over the whole list; what changed is the switch that was pressed.
        const changed = [...next.filter((v) => !on.includes(v as V)), ...on.filter((v) => !next.includes(v))];
        if (changed.length > 0) onToggle(changed[0] as V);
      }}
    >
      {segments.map((s) => (
        <Toggle key={s.value} value={s.value} title={s.title} disabled={s.disabled} className="segment">
          {s.label}
        </Toggle>
      ))}
    </ToggleGroup>
  );
}

type ChoiceProps<V extends string> = {
  segments: Segment<V>[];
  value: V;
  onChange(value: V): void;
  label: string;
  className?: string;
};

export function SegmentedChoice<V extends string>({ segments, value, onChange, label, className = "" }: ChoiceProps<V>) {
  return (
    <RadioGroup
      value={value}
      aria-label={label}
      className={`segmented ${className}`.trim()}
      onValueChange={(next) => onChange(next as V)}
    >
      {segments.map((s) => (
        <Radio.Root
          key={s.value}
          value={s.value}
          title={s.title}
          disabled={s.disabled}
          nativeButton
          render={<button type="button" />}
          className="segment"
        >
          {s.label}
        </Radio.Root>
      ))}
    </RadioGroup>
  );
}
