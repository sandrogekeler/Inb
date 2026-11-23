# Inb: roadmap

Intent, not record. What is done lives in the code and the closed issues; this
file says what is next and why. Unchecked items are not done.

## Now

The first build, one issue per part. Each can be built and tried on its own:

- [ ] Draw the frame, the bar and the fps menu (#8).
- [ ] Show the windows on Windows (#9) and macOS (#10).
- [ ] Record the area on Windows (#11) and macOS (#12).
- [ ] Small, smooth GIFs from long recordings (#13).
- [ ] Copy the GIF to the clipboard and save it (#14).

## Next

- [ ] Package Inb as an app: a signed `.app` on macOS (the Screen Recording
      permission then belongs to Inb, not the terminal) and an `.exe` with an
      icon and version info on Windows.
- [ ] Set the budgets in the health checklist from measurements: startup,
      memory while recording, GIF size per second.

## Later

- [ ] Trim a recording before saving it.
- [ ] Linux. X11 works like the other two systems. On Wayland an app cannot
      place its own windows, and GNOME lacks the layer-shell protocol that
      allows it on KDE and Sway, so the frame needs another design there.
      Capture goes through the xdg-desktop-portal screen-share picker.

## Ideas

Not decided yet. An idea that is decided becomes an issue.
