// Helpers for native <dialog> windows.

/// For a dialog's `onclick`: a click on the backdrop (the dialog element
/// itself, outside its content) closes it.
export function closeOnBackdrop(e: MouseEvent, dialog: HTMLDialogElement | null | undefined) {
  if (dialog && e.target === dialog) dialog.close();
}
