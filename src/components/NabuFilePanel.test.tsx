import { BlockNoteContext, ComponentsContext } from "@blocknote/react";
import {
	act,
	fireEvent,
	render,
	screen,
	waitFor,
} from "@testing-library/react";
import type { ReactNode } from "react";
import { describe, expect, it, vi } from "vitest";
import { NabuFilePanel } from "./NabuFilePanel";

const filePanelComponents = {
	FilePanel: {
		Root: ({ tabs }: { tabs: Array<{ tabPanel: ReactNode }> }) =>
			tabs[0]?.tabPanel ?? null,
		Button: ({ children }: { children?: ReactNode }) => <>{children}</>,
		FileInput: ({
			onChange,
			placeholder,
		}: {
			onChange: (file: File) => void;
			placeholder: string;
		}) => (
			<button
				type="button"
				onClick={() =>
					onChange(new File(["image"], "image.png", { type: "image/png" }))
				}
			>
				{placeholder}
			</button>
		),
		TabPanel: ({ children }: { children?: ReactNode }) => <>{children}</>,
		TextInput: () => null,
	},
};

function panel(editor: Record<string, unknown>) {
	return (
		<ComponentsContext.Provider value={filePanelComponents as never}>
			<BlockNoteContext.Provider
				value={{ editor: editor as never, setContentEditableProps: vi.fn() }}
			>
				<NabuFilePanel blockId="image-block" />
			</BlockNoteContext.Provider>
		</ComponentsContext.Provider>
	);
}

describe("NabuFilePanel", () => {
	it("ignores a completed upload when a note reload removed its block", async () => {
		let currentBlock: { id: string; type: string } | undefined = {
			id: "image-block",
			type: "image",
		};
		let finishUpload: (value: string) => void = () => undefined;
		const uploadFile = vi.fn(
			() =>
				new Promise<string>((resolve) => {
					finishUpload = resolve;
				}),
		);
		const updateBlock = vi.fn();
		const editor = {
			dictionary: {
				file_panel: {
					embed: { title: "Embed" },
					upload: {
						file_placeholder: { file: "Choose file", image: "Choose image" },
						title: "Upload",
						upload_error: "Upload failed",
					},
				},
			},
			getBlock: vi.fn(() => currentBlock),
			schema: {
				blockSpecs: {
					image: { implementation: { meta: { fileBlockAccept: ["image/*"] } } },
				},
			},
			updateBlock,
			uploadFile,
		};
		const { rerender } = render(panel(editor));

		fireEvent.click(screen.getByRole("button", { name: "Choose image" }));
		expect(uploadFile).toHaveBeenCalledWith(expect.any(File), "image-block");

		currentBlock = undefined;
		rerender(panel(editor));
		expect(screen.queryByText("Choose image")).not.toBeInTheDocument();

		await act(async () => finishUpload("/vault/attachments/image.png"));
		await waitFor(() => expect(updateBlock).not.toHaveBeenCalled());
	});
});
