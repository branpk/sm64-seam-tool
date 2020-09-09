# Seam Tool

Program for finding gaps and overlaps between two walls in Super Mario 64.

Download and extract the .zip file from "Releases" on the right, and double click the .exe to open.

## World view

The first time you enter an area, the program has to detect gaps and overlaps in all the seams within the area. The number of seams left to check is shown at the top left next to "Remaining". This progress is saved when you leave and re-enter an area, but not when you exit the program. This process should not take longer than 10 minutes for any area - please log a bug if it takes longer.

Colors:
- Blue = overlap, no gap
- Green = gap, no overlap
- Cyan = both gap and overlap
- White = neither
- Dark grey = not yet checked
- Red = skipped - points in the range [-1, 1] are skipped for performance reasons

If you care about object surfaces and the flickering is bothersome, enable the "Sync" checkbox, which should mitigate it a little.

You can filter y values using the dropdown:
- "all y": no filtering
- "int y": only gaps/overlaps at integer height are shown
- "qint y": only gaps/overlaps with fractional part equal to .00, .25, .50, or .75 are shown.

## Seam view

Clicking on a seam opens it in another pane. From there, you can zoom in and drag using the mouse to get a closer look at the gaps and overlaps. If you zoom in enough, the gaps/overlaps will be shown as individual points instead of ranges, and eventually the grid of discrete floats will be visible.

Note that when you drag or zoom, it may take a couple seconds to update.

The "Export" button allows you to save seam data to a CSV. The file will be saved in the same folder as the .exe using the provided filename. The rest of the program ignores the range [-1, 1], but it can optionally be included when exporting. You can also choose to include only gaps, only overlaps, both, or all points. If you choose all points, note that the listed y values are not particularly meaningful.

After exporting, the "Export" button is replaced with a message that shows the progress of the export. You can close the seam view or switch seams and the export will continue in the background, but closing the program will interrupt it.

**Warning**: If you export a seam close to the origin and you include [-1, 1], the resulting file may be huge (over 100 GB). The only way to kill an export is to close the program.

## Other game versions and emulators

Support for other game versions, rom hacks, and emulators can be added by editing config.json with the appropriate values. Feel free to submit a pull request with these changes.
