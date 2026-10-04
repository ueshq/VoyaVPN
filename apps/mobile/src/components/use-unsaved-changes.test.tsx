import { NavigationContainer, useNavigation } from "@react-navigation/native";
import { createNativeStackNavigator } from "@react-navigation/native-stack";
import { act, fireEvent, render, screen, waitFor } from "@testing-library/react-native";
import { useState } from "react";
import { Alert, type AlertButton, Pressable, Text } from "react-native";

import { useBusyAction } from "./use-busy-action";
import { useUnsavedChanges } from "./use-unsaved-changes";

const Stack = createNativeStackNavigator();
const saveEdits = jest.fn<Promise<boolean>, []>();

function List() {
  const navigation = useNavigation();
  return <Pressable onPress={() => navigation.navigate("editor" as never)}><Text>Open editor</Text></Pressable>;
}

/** An editor in the shape the real ones have: a busy guard around a save that clears the edit. */
function Editor() {
  const navigation = useNavigation();
  const [dirty, setDirty] = useState(false);
  const { busy, run } = useBusyAction();
  const save = async () =>
    (await run(async () => {
      const saved = await saveEdits();
      if (saved) setDirty(false);
      return saved;
    })) ?? false;
  useUnsavedChanges(dirty, busy, save);

  return (
    <>
      <Text>Editor</Text>
      <Pressable onPress={() => setDirty(true)}><Text>Edit</Text></Pressable>
      <Pressable onPress={() => navigation.goBack()}><Text>Back</Text></Pressable>
    </>
  );
}

/** Opens the editor, edits, asks to leave, and returns the prompt's buttons by label. */
async function leaveWithEdits() {
  const alert = jest.spyOn(Alert, "alert").mockImplementation(() => {});
  await render(
    <NavigationContainer>
      <Stack.Navigator screenOptions={{ animation: "none", headerShown: false }}>
        <Stack.Screen name="list" component={List} />
        <Stack.Screen name="editor" component={Editor} />
      </Stack.Navigator>
    </NavigationContainer>,
  );
  await fireEvent.press(screen.getByText("Open editor"));
  await fireEvent.press(await screen.findByText("Edit"));
  await fireEvent.press(screen.getByText("Back"));
  expect(alert).toHaveBeenCalledTimes(1);
  const buttons = alert.mock.calls[0]?.[2] ?? [];

  return (label: string) => buttons.find((button: AlertButton) => button.text === label)?.onPress?.();
}

describe("leaving a page with unsaved edits", () => {
  beforeEach(() => {
    saveEdits.mockReset();
  });

  afterEach(() => {
    jest.restoreAllMocks();
  });

  it("stays on the page until the user answers", async () => {
    await leaveWithEdits();

    expect(screen.getByText("Editor")).toBeOnTheScreen();
  });

  it("saves and then leaves when the prompt's Save is chosen", async () => {
    saveEdits.mockResolvedValue(true);
    const choose = await leaveWithEdits();

    await act(async () => choose("Save"));

    expect(saveEdits).toHaveBeenCalledTimes(1);
    await waitFor(() => expect(screen.queryByText("Editor")).toBeNull());
    expect(screen.getByText("Open editor")).toBeOnTheScreen();
  });

  it("stays when the save fails", async () => {
    saveEdits.mockResolvedValue(false);
    const choose = await leaveWithEdits();

    await act(async () => choose("Save"));

    expect(screen.getByText("Editor")).toBeOnTheScreen();
  });

  it("leaves without saving when the edits are discarded", async () => {
    const choose = await leaveWithEdits();

    await act(async () => choose("Discard changes"));

    await waitFor(() => expect(screen.queryByText("Editor")).toBeNull());
    expect(saveEdits).not.toHaveBeenCalled();
  });
});
