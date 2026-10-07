import { Button } from "heroui-native/button";

/**
 * A screen's primary action.
 *
 * HeroUI dims a disabled button, which on the solid accent still reads as the
 * thing to press. While disabled this one takes the muted fill and label
 * instead — see `.button--primary-disabled` in global.css — so "nothing to
 * save yet" looks unavailable.
 */
export function PrimaryButton({
  isDisabled = false,
  label,
  onPress,
}: {
  isDisabled?: boolean;
  label: string;
  onPress: () => void;
}) {
  return (
    <Button isDisabled={isDisabled} className={isDisabled ? "button--primary-disabled" : undefined} onPress={onPress}>
      <Button.Label className={isDisabled ? "text-subtle" : undefined}>{label}</Button.Label>
    </Button>
  );
}
