import { ControlField } from "heroui-native/control-field";
import { Label } from "heroui-native/label";
import { Separator } from "heroui-native/separator";
import { Switch } from "heroui-native/switch";
import { View } from "react-native";

/**
 * A setting that is on or off, as one row of a `ListGroup`: HeroUI's
 * `ControlField`, so a tap anywhere on the row flips the switch.
 *
 * The field's root is a `Pressable`, and React Native makes a `Pressable` an
 * accessibility element of its own — on iOS that swallows the switch inside
 * it, so VoiceOver reads a plain label with no on/off state and XCUITest finds
 * no `switches[label]`. `accessible={false}` on the root keeps the switch the
 * element that is announced and tested, under its own label.
 */
export function SwitchRow({
  isDisabled,
  label,
  last = false,
  onChange,
  value,
}: {
  isDisabled?: boolean;
  label: string;
  last?: boolean;
  onChange: (value: boolean) => void;
  value: boolean;
}) {
  return (
    <View>
      <ControlField
        accessible={false}
        className="p-4"
        isDisabled={isDisabled}
        isSelected={value}
        onSelectedChange={onChange}
      >
        <Label className="min-w-0 flex-1">{label}</Label>
        <ControlField.Indicator>
          <Switch accessibilityLabel={label} hitSlop={10} />
        </ControlField.Indicator>
      </ControlField>
      {last ? null : <Separator accessible={false} className="ml-4" />}
    </View>
  );
}
