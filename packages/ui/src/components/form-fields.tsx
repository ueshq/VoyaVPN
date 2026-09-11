import { useId, type ReactNode } from "react";

import { Checkbox } from "./checkbox";
import { Input } from "./input";
import { Label } from "./label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "./select";
import { Textarea } from "./textarea";
import { cn } from "../lib/utils";
import { useCommittedInput } from "../lib/committed-input";

type FieldLayoutProps = {
  children: ReactNode;
  className?: string;
  description?: ReactNode;
  error?: string;
  group?: boolean;
  id: string;
  label?: ReactNode;
  layout?: "stack" | "row";
};

export function FieldLayout({ children, className, description, error, group = false, id, label, layout = "stack" }: FieldLayoutProps) {
  return (
    <div role={group ? "group" : undefined} aria-labelledby={group && label ? `${id}-label` : undefined} aria-describedby={group ? descriptionIds(id, description, error) : undefined} className={cn("grid min-w-0 gap-1.5", layout === "row" && label && "@min-[42rem]:grid-cols-[minmax(12rem,1fr)_minmax(0,22rem)] @min-[42rem]:items-start @min-[42rem]:gap-x-6", className)}>
      {label ? <div className={cn("grid min-w-0 gap-1", layout === "row" && "py-1.5")}>
        <Label className="leading-5" htmlFor={group ? undefined : id} id={`${id}-label`}>{label}</Label>
        {description ? <p className="text-xs text-muted-foreground" id={`${id}-description`}>{description}</p> : null}
      </div> : null}
      <div className="grid min-w-0 gap-1.5">
        {children}
        {error ? <span className="text-xs text-danger" id={`${id}-error`}>{error}</span> : null}
      </div>
    </div>
  );
}

type FieldProps = Omit<FieldLayoutProps, "children" | "id"> & {
  disabled?: boolean;
  id?: string;
  onChange: (value: string) => void;
  value: string;
};

type TextFieldProps = FieldProps & {
  commitOnBlur?: boolean;
  inputMode?: "numeric";
  validate?: (value: string) => string | undefined;
  onInvalid?: (message: string, leaving: boolean) => void;
};

function descriptionIds(id: string, description: ReactNode, error: string | undefined) {
  return [description ? `${id}-description` : null, error ? `${id}-error` : null].filter(Boolean).join(" ") || undefined;
}

function TextInputField({
  className, commitOnBlur = false, description, disabled, error, id, inputMode, label, layout, multiline, onChange, onInvalid, validate, value,
}: TextFieldProps & { multiline?: boolean }) {
  const generatedId = useId();
  const inputId = id ?? generatedId;
  const input = useCommittedInput({ value, onChange, deferred: commitOnBlur, validate, onInvalid });
  const { error: inputError, ...inputProps } = input;
  const message = inputError ?? error;
  const Control = multiline ? Textarea : Input;

  return (
    <FieldLayout className={className} description={description} error={message} id={inputId} label={label} layout={layout}>
      <Control
        {...inputProps}
        aria-describedby={descriptionIds(inputId, description, message)}
        aria-invalid={message ? true : undefined}
        className={cn(multiline ? "min-h-24 resize-y" : undefined, layout === "row" && !multiline && "h-8")}
        disabled={disabled}
        id={inputId}
        inputMode={inputMode}
        onChange={(event) => input.onChange(event.target.value)}
      />
    </FieldLayout>
  );
}

export function TextField(props: TextFieldProps) { return <TextInputField {...props} />; }
export function TextAreaField(props: TextFieldProps) { return <TextInputField {...props} multiline />; }

const EMPTY_SELECT_VALUE = "__voyavpn_empty_select_value__";

export function SelectField({
  className, description, disabled, error, id, label, layout, onChange, options, value,
}: FieldProps & { options: ReadonlyArray<{ label: string; value: string }> }) {
  const generatedId = useId();
  const inputId = id ?? generatedId;

  return (
    <FieldLayout className={className} description={description} error={error} id={inputId} label={label} layout={layout}>
      <Select disabled={disabled} onValueChange={(next) => onChange(next === EMPTY_SELECT_VALUE ? "" : next)} value={value === "" ? EMPTY_SELECT_VALUE : value}>
        <SelectTrigger aria-describedby={descriptionIds(inputId, description, error)} aria-invalid={error ? true : undefined} className={cn("w-full", layout === "row" && "h-8")} id={inputId}>
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          {options.map((option) => <SelectItem key={option.value || EMPTY_SELECT_VALUE} value={option.value || EMPTY_SELECT_VALUE}>{option.label}</SelectItem>)}
        </SelectContent>
      </Select>
    </FieldLayout>
  );
}

export function CheckboxField({ checked, className, description, disabled, error, id, label, onChange }: {
  checked: boolean;
  className?: string;
  description?: ReactNode;
  disabled?: boolean;
  error?: string;
  id?: string;
  label: ReactNode;
  onChange: (value: boolean) => void;
}) {
  const generatedId = useId();
  const inputId = id ?? generatedId;
  return (
    <div className={cn("flex items-start gap-2", disabled && "opacity-55", className)}>
      <Checkbox aria-describedby={descriptionIds(inputId, description, error)} aria-invalid={error ? true : undefined} className="mt-0.5" checked={checked} disabled={disabled} id={inputId} onCheckedChange={(next) => onChange(next === true)} />
      <div className="grid min-w-0 gap-1">
        <Label className={cn("text-sm leading-5", disabled ? "cursor-not-allowed" : "cursor-pointer")} htmlFor={inputId}>{label}</Label>
        {description ? <p className="text-xs text-muted-foreground" id={`${inputId}-description`}>{description}</p> : null}
        {error ? <span className="text-xs text-danger" id={`${inputId}-error`}>{error}</span> : null}
      </div>
    </div>
  );
}
