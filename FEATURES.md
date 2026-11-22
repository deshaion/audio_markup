# Audio Markup features

Audio Markup opens a local audio file, lets you mark track starts, and splits the recording into tagged MP3s.

## Audio playback

| Control | Behavior |
| --- | --- |
| **Open Audio…** | Selects one local file, begins playback, and displays the path. Opening another file keeps the current markers. |
| **Play / Pause** | Resumes or pauses playback. Audio loops at the end. |
| **−10 s / +10 s** | Seeks backward or forward by 10 seconds per press. Seeks stay within the audio duration when the backend reports it. |
| **Position readout** | Shows the playhead and total duration at whole-second precision. |
| **Volume** | Adjusts output from 0% to 100%, starting at 100%, in 10-percentage-point steps. |

## Marker table

- Edit **Pos**, **Artist**, and **Title** directly in a scrollable table. The header stays visible while scrolling.
- Start with one marker at `00:00`. **Add marker** appends a blank row and **Delete** removes a row. Markers live in the current session and can be saved as a CUE sheet.
- Select a row by clicking or focusing a cell. Use **Set from playhead** to capture the current time or **Seek to marker** to jump to the row’s saved position.
- Enter a timestamp as `MM:SS`, `MM:SS.mmm`, or `MM:SS:FF` for CUE frame precision. Invalid timestamps are marked in the table and block export until corrected.
- Paste `Artist - Title` into the **Artist** cell to fill both metadata fields.

## Keyboard shortcuts

| Shortcut | Action |
| --- | --- |
| `Alt+↑` | Resume playback. |
| `Alt+↓` | Pause playback. |
| `Alt+←` | Seek back 10 seconds. |
| `Alt+→` | Seek forward 10 seconds. |
| `Alt+X` | Write the current playhead time into the selected row. |
| `Alt+S` | Seek to the selected row’s timestamp. |

## CUE sheets

- **Import CUE…** reads a UTF-8, single-audio-file CUE sheet. It opens the referenced audio and fills the table using each track’s `INDEX 01`, `PERFORMER`, and `TITLE`. Tracks with no performer inherit the album performer. If the audio moved, the app lets you locate it. Replacing edited marker rows requires confirmation.
- **Export CUE…** saves all timestamped markers in track order with artist and title metadata and a reference to the current audio. Timestamps are stored at the CUE standard’s 75 frames per second. [CUE format reference](https://www.gnu.org/software/ccd2cue/manual/ccd2cue.pdf)

## Split to MP3

1. Choose **Split to MP3…** and select an output folder. Each row with an artist or title produces a file. An untagged row can still set the end of the preceding track. An eligible final row runs to the end of the recording.
2. Outputs are named `Artist - Title.mp3` and contain the corresponding tags. FFmpeg copies MP3 input streams and encodes other sources with `libmp3lame` at quality setting `2`. Existing output files are protected.
3. The app shows progress during export and reports failed segments. On success it displays **Split is completed** as an in-app notification.
