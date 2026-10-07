import { Pressable, Text, View, type ViewProps } from "react-native";

/** The RNCTabView native event boundary; the library and navigation router stay real. */
type NativeTabProps = ViewProps & {
  items: readonly { key: string; title: string; hidden?: boolean; preventsDefault?: boolean }[];
  selectedPage: string;
  tabBarHidden?: boolean;
  onPageSelected?: (event: { nativeEvent: { key: string } }) => void;
};

export function NativeTabsMock({ children, items, selectedPage, tabBarHidden, onPageSelected }: NativeTabProps) {
  return (
    <View>
      {children}
      {!tabBarHidden && items.filter((item) => !item.hidden).map((item) => (
        <Pressable
          key={item.key}
          accessibilityRole="tab"
          accessibilityLabel={item.title}
          accessibilityState={{ selected: selectedPage === item.key }}
          disabled={item.preventsDefault}
          onPress={() => onPageSelected?.({ nativeEvent: { key: item.key } })}
        >
          <Text>{item.title}</Text>
        </Pressable>
      ))}
    </View>
  );
}
