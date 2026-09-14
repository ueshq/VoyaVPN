import { useState } from "react";
import { useI18n } from "@voya/i18n/use-i18n";
import { profileValidationMessage } from "./profile-form-utils";
import { createContext, useContext, useId } from "react";
import type * as React from "react";
import { Controller } from "react-hook-form";
import type {
  Control,
  FieldErrors,
  FieldPath,
  UseFormRegister,
} from "react-hook-form";

import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
} from "@voya/ui/components/card";
import { Checkbox } from "@voya/ui/components/checkbox";
import { Input } from "@voya/ui/components/input";
import { Label } from "@voya/ui/components/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@voya/ui/components/select";
import { cn } from "@voya/ui/lib/utils";
import { ChevronDown, ShieldCheck } from "lucide-react";

import type {
  ParsedProfileFormValues,
  ProfileFormValues,
} from "./profile-form-schema";

const FieldErrorsContext = createContext<FieldErrors<ProfileFormValues>>({});

export function ProfileFields({
  errors,
  children,
}: {
  errors: FieldErrors<ProfileFormValues>;
  children: React.ReactNode;
}) {
  return <FieldErrorsContext value={errors}>{children}</FieldErrorsContext>;
}

export type Register = UseFormRegister<ProfileFormValues>;
export type ProfileFormControl = Control<
  ProfileFormValues,
  unknown,
  ParsedProfileFormValues
>;

export function Panel({
  children,
  collapsible = false,
  defaultOpen = true,
  title,
}: {
  children: React.ReactNode;
  /** A collapsible panel can start closed while it has nothing worth showing. */
  collapsible?: boolean;
  defaultOpen?: boolean;
  title: string;
}) {
  if (collapsible) {
    return (
      <CollapsiblePanel defaultOpen={defaultOpen} title={title}>
        {children}
      </CollapsiblePanel>
    );
  }
  return (
    <Card className="gap-3 rounded-xl bg-surface-raised p-3 shadow-raised">
      <CardHeader className="p-0">
        <CardTitle className="flex items-center gap-2 text-section font-semibold text-foreground">
          <ShieldCheck
            className="size-4 text-muted-foreground"
            aria-hidden="true"
          />
          {title}
        </CardTitle>
      </CardHeader>
      <CardContent className="p-0">{children}</CardContent>
    </Card>
  );
}

// A native <details>, so the form's error handler that opens every section
// also reaches this one.
function CollapsiblePanel({
  children,
  defaultOpen,
  title,
}: {
  children: React.ReactNode;
  defaultOpen: boolean;
  title: string;
}) {
  // Only the first render decides; filling the panel in never snaps it shut.
  const [initiallyOpen] = useState(defaultOpen);
  return (
    <Card className="gap-3 rounded-xl bg-surface-raised p-3 shadow-raised">
      <details className="group/panel" open={initiallyOpen}>
        <summary className="flex cursor-pointer list-none items-center gap-2 text-section font-semibold text-foreground [&::-webkit-details-marker]:hidden">
          <ShieldCheck className="size-4 text-muted-foreground" aria-hidden="true" />
          {title}
          <ChevronDown
            aria-hidden="true"
            className="ms-auto size-4 text-muted-foreground transition-transform group-open/panel:rotate-180"
          />
        </summary>
        <div className="pt-3">{children}</div>
      </details>
    </Card>
  );
}

type TextFieldProps = React.InputHTMLAttributes<HTMLInputElement> & {
  error?: string;
  label: string;
};

export function TextField({
  className,
  error,
  id,
  label,
  ...props
}: TextFieldProps) {
  const errors = useContext(FieldErrorsContext);
  const { t } = useI18n();
  let issue: unknown = errors;
  for (const part of (props.name ?? "").split(".")) {
    issue =
      issue && typeof issue === "object"
        ? (issue as Record<string, unknown>)[part]
        : undefined;
  }
  if (
    !error &&
    issue &&
    typeof issue === "object" &&
    "message" in issue &&
    typeof issue.message === "string"
  ) {
    error = profileValidationMessage(issue.message, t);
  }

  // Ids are generated, never derived from the label: a translated label such as
  // "备注" contains no ASCII word characters, so a slugified id collapsed to the
  // empty string and broke label/input associations for non-Latin labels.
  const generatedId = useId();
  const inputId = id ?? generatedId;
  const errorId = `${inputId}-error`;
  const {
    "aria-describedby": ariaDescribedBy,
    "aria-invalid": ariaInvalid,
    ...inputProps
  } = props;

  return (
    <div className="grid min-w-0 gap-1">
      <Label className="text-xs text-muted-foreground" htmlFor={inputId}>
        <span className="truncate">{label}</span>
      </Label>
      <Input
        aria-describedby={
          error ? mergeIds(ariaDescribedBy, errorId) : ariaDescribedBy
        }
        aria-invalid={error ? true : ariaInvalid}
        className={cn("bg-card", className)}
        id={inputId}
        {...inputProps}
      />
      {error ? (
        <span className="text-xs text-danger" id={errorId}>
          {error}
        </span>
      ) : null}
    </div>
  );
}

type SelectOption = {
  description?: string;
  label: string;
  value: number | string;
};

type SelectFieldProps = {
  className?: string;
  control: ProfileFormControl;
  error?: string;
  id?: string;
  label: string;
  name: FieldPath<ProfileFormValues>;
  onValueChange?: (value: string) => void;
  options: SelectOption[];
  parseValue?: (value: string) => unknown;
};

export function SelectField({
  className,
  control,
  error,
  id,
  label,
  name,
  onValueChange,
  options,
  parseValue,
}: SelectFieldProps) {
  const generatedId = useId();
  const inputId = id ?? generatedId;
  const errorId = `${inputId}-error`;

  return (
    <Controller
      control={control}
      name={name}
      render={({ field, fieldState }) => {
        const invalid = Boolean(error ?? fieldState.error?.message);

        return (
          <div className="grid min-w-0 gap-1">
            <Label className="text-xs text-muted-foreground" htmlFor={inputId}>
              <span className="truncate">{label}</span>
            </Label>
            <Select
              name={field.name}
              onValueChange={(value) => {
                const decoded = decodeSelectValue(value);

                field.onChange(parseValue ? parseValue(decoded) : decoded);
                onValueChange?.(decoded);
              }}
              value={encodeSelectValue(field.value)}
            >
              <SelectTrigger
                aria-describedby={error ? errorId : undefined}
                aria-invalid={invalid ? true : undefined}
                className={cn("w-full bg-card", className)}
                id={inputId}
                onBlur={field.onBlur}
                ref={field.ref}
              >
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {options.map((option) => (
                  <SelectItem
                    key={`${name}-${option.value}`}
                    value={encodeSelectValue(option.value)}
                  >
                    <span>{option.label}</span>
                    {option.description ? (
                      <span className="sr-only">{option.description}</span>
                    ) : null}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
            {error ? (
              <span className="text-xs text-danger" id={errorId}>
                {error}
              </span>
            ) : null}
          </div>
        );
      }}
    />
  );
}

type CheckboxFieldProps = {
  className?: string;
  control: ProfileFormControl;
  id?: string;
  label: string;
  name: FieldPath<ProfileFormValues>;
};

export function CheckboxField({
  className,
  control,
  id,
  label,
  name,
}: CheckboxFieldProps) {
  const generatedId = useId();
  const inputId = id ?? generatedId;

  return (
    <Controller
      control={control}
      name={name}
      render={({ field, fieldState }) => (
        <Card
          className={cn(
            "h-9 min-w-0 justify-center gap-0 rounded-md bg-card px-3 py-0 shadow-none",
            className,
          )}
        >
          <Label
            className="h-full w-full min-w-0 cursor-pointer text-xs font-medium text-muted-foreground"
            htmlFor={inputId}
          >
            <Checkbox
              aria-invalid={fieldState.invalid ? true : undefined}
              checked={field.value === true}
              id={inputId}
              name={field.name}
              onBlur={field.onBlur}
              onCheckedChange={(checked) => field.onChange(checked === true)}
              ref={field.ref}
            />
            <span className="truncate">{label}</span>
          </Label>
        </Card>
      )}
    />
  );
}

const EMPTY_SELECT_VALUE = "__voyavpn_empty__";

function encodeSelectValue(value: unknown) {
  const stringValue =
    value === null || value === undefined ? "" : String(value);

  return stringValue === "" ? EMPTY_SELECT_VALUE : stringValue;
}

function decodeSelectValue(value: string) {
  return value === EMPTY_SELECT_VALUE ? "" : value;
}

function mergeIds(...ids: Array<string | undefined>) {
  return ids.filter(Boolean).join(" ") || undefined;
}
