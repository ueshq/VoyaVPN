import { useId } from "react";

import { Checkbox } from "./checkbox";
import { Input } from "./input";
import { Label } from "./label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "./select";
import { Textarea } from "./textarea";
import { cn } from "../lib/utils";

type FieldProps = {
  className?: string;
  disabled?: boolean;
  error?: string;
  id?: string;
  label: string;
  onChange: (value: string) => void;
  value: string;
};

function TextInputField({
  className, disabled, error, id, label, multiline, onChange, value,
}: FieldProps & { multiline?: boolean }) {
  const generatedId = useId();
  const inputId = id ?? generatedId;
  const errorId = `${inputId}-error`;
  const Control = multiline ? Textarea : Input;

  return (
    <div className={cn("grid gap-1.5", className)}>
      <Label htmlFor={inputId}>{label}</Label>
      <Control
        aria-describedby={error ? errorId : undefined}
        aria-invalid={error ? true : undefined}
        className={multiline ? "min-h-24 resize-y" : undefined}
        disabled={disabled}
        id={inputId}
        onChange={(event) => onChange(event.target.value)}
        value={value}
      />
      {error ? <span className="text-xs text-destructive" id={errorId}>{error}</span> : null}
    </div>
  );
}

export function TextField(props: FieldProps) {
  return <TextInputField {...props} />;
}

export function TextAreaField(props: FieldProps) {
  return <TextInputField {...props} multiline />;
}

const EMPTY_SELECT_VALUE = "__voyavpn_empty_select_value__";

export function SelectField({
  className, disabled, error, id, label, onChange, options, value,
}: FieldProps & {
  options: ReadonlyArray<{ label: string; value: string }>;
}) {
  const generatedId = useId();
  const inputId = id ?? generatedId;
  const errorId = `${inputId}-error`;

  return (
    <div className={cn("grid gap-1.5", className)}>
      <Label htmlFor={inputId}>{label}</Label>
      <Select
        disabled={disabled}
        onValueChange={(nextValue) => onChange(nextValue === EMPTY_SELECT_VALUE ? "" : nextValue)}
        value={value === "" ? EMPTY_SELECT_VALUE : value}
      >
        <SelectTrigger
          aria-describedby={error ? errorId : undefined}
          aria-invalid={error ? true : undefined}
          className="w-full"
          id={inputId}
        >
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          {options.map((option) => (
            <SelectItem key={option.value || EMPTY_SELECT_VALUE} value={option.value || EMPTY_SELECT_VALUE}>
              {option.label}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
      {error ? <span className="text-xs text-destructive" id={errorId}>{error}</span> : null}
    </div>
  );
}

export function CheckboxField({ checked, className, disabled, id, label, onChange }: {
  checked: boolean;
  className?: string;
  disabled?: boolean;
  id?: string;
  label: string;
  onChange: (value: boolean) => void;
}) {
  const generatedId = useId();
  const inputId = id ?? generatedId;

  return (
    <div className={cn("flex items-center gap-2", disabled && "opacity-55", className)}>
      <Checkbox
        checked={checked}
        disabled={disabled}
        id={inputId}
        onCheckedChange={(next) => onChange(next === true)}
      />
      <Label className={cn("text-sm", disabled ? "cursor-not-allowed" : "cursor-pointer")} htmlFor={inputId}>
        {label}
      </Label>
    </div>
  );
}
