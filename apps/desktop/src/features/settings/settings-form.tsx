import {
  createContext,
  useContext,
  useId,
  type ComponentProps,
  type ReactNode,
  type Ref,
} from "react";

import {
  FieldLayout,
  SelectField as SharedSelectField,
  SwitchField,
  TextField as SharedTextField,
} from "@voya/ui/components/form-fields";
import { cn } from "@voya/ui/lib/utils";
import { useI18n } from "@voya/i18n/use-i18n";
import { pageSurfaceClassName } from "@/components/app-shell/page-section";

const FieldErrors = createContext<Record<string, string>>({});

export function SettingsFields({
  children,
  errors,
}: {
  children: ReactNode;
  errors: Record<string, string>;
}) {
  return <FieldErrors value={errors}>{children}</FieldErrors>;
}

export function SettingsGroup({
  children,
  className,
  title,
  actions,
  headingRef,
}: {
  children: ReactNode;
  className?: string;
  title: string;
  actions?: ReactNode;
  headingRef?: Ref<HTMLHeadingElement>;
}) {
  const id = useId();
  return (
    <section
      aria-labelledby={id}
      className={cn(
        pageSurfaceClassName,
        "@container",
        className,
      )}
    >
      <div className="flex min-h-11 flex-wrap items-end gap-2 px-4 pt-3">
        <h2 className="text-section font-semibold outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2" id={id} ref={headingRef} tabIndex={headingRef ? -1 : undefined}>
          {title}
        </h2>
        {actions ? (
          <div className="ms-auto flex items-center gap-2">{actions}</div>
        ) : null}
      </div>
      {/* One setting per row, divided by hairlines, so every control reads
          with its own label in one shared control column. */}
      <div className="grid divide-y divide-border-subtle px-4 pb-1 [&>*]:py-3">
        {children}
      </div>
    </section>
  );
}

export function SettingsRow({
  children,
  className,
  description,
  htmlFor,
  label,
  error,
}: {
  children: ReactNode;
  className?: string;
  description?: ReactNode;
  htmlFor?: string;
  label?: ReactNode;
  error?: string;
}) {
  const id = useId();
  return (
    <FieldLayout
      className={className}
      description={description}
      error={error}
      group={!htmlFor}
      id={htmlFor ?? id}
      label={label}
      layout="row"
    >
      {children}
    </FieldLayout>
  );
}

/** An on/off setting that saves at once, shown as a switch at the row's end. */
export function SettingsSwitch({
  field,
  onCheckedChange,
  ...props
}: Omit<ComponentProps<typeof SwitchField>, "onChange"> & {
  field?: string;
  onCheckedChange: (checked: boolean) => void;
}) {
  const errors = useContext(FieldErrors);
  return (
    <SwitchField
      {...props}
      error={field ? errors[field] : undefined}
      onChange={onCheckedChange}
    />
  );
}

type TextProps = Omit<
  ComponentProps<typeof SharedTextField>,
  "layout" | "commitOnBlur"
> & { field: string };

export function TextField({ field, ...props }: TextProps) {
  const errors = useContext(FieldErrors);
  return (
    <SharedTextField
      {...props}
      commitOnBlur
      error={errors[field]}
      layout="row"
    />
  );
}

export function NumberField({
  defaultValue,
  description,
  nullable = false,
  onChange,
  value,
  ...props
}: Omit<TextProps, "description" | "onChange" | "value"> & {
  /** Restored when the field is cleared, and named under the field. */
  defaultValue?: number;
  description?: string;
  nullable?: boolean;
  onChange: (value: number | null) => void;
  value: number | null;
}) {
  const { t } = useI18n();
  const hint =
    defaultValue === undefined
      ? description
      : [description, t("settings.defaultValue", { value: defaultValue })]
          .filter(Boolean)
          .join(" ");
  // A typing mistake stays next to the field; it is not a failed save, so it
  // never raises a toast.
  return (
    <TextField
      {...props}
      description={hint}
      inputClassName="w-36 max-w-full"
      inputMode="numeric"
      onChange={(text) =>
        onChange(text.trim() ? Number(text) : (defaultValue ?? null))
      }
      validate={(text) => {
        if (!text.trim())
          return nullable || defaultValue !== undefined
            ? undefined
            : t("validation.textRequired");
        const number = Number(text);
        return Number.isInteger(number) &&
          number >= -2147483648 &&
          number <= 2147483647
          ? undefined
          : t("validation.invalid");
      }}
      value={value === null ? "" : String(value)}
    />
  );
}

export function SelectField({
  field,
  optionLabel = (value) => value,
  options,
  ...props
}: Omit<ComponentProps<typeof SharedSelectField>, "options" | "layout"> & {
  field: string;
  optionLabel?: (value: string) => string;
  options: readonly string[];
}) {
  const errors = useContext(FieldErrors);
  return (
    <SharedSelectField
      {...props}
      error={errors[field]}
      layout="row"
      options={options.map((value) => ({ value, label: optionLabel(value) }))}
    />
  );
}
