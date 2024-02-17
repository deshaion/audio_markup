
import 'package:audioplayers/audioplayers.dart';
import 'package:file_picker/file_picker.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'dart:io';

import 'editable.dart';

void main() {
  runApp(const MyApp());
}

class MyApp extends StatelessWidget {
  const MyApp({super.key});

  // This widget is the root of your application.
  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'Audio Markup',
      theme: ThemeData(
        colorScheme: ColorScheme.fromSeed(seedColor: Colors.deepPurple),
        useMaterial3: true,
      ),
      home: const MyHomePage(title: 'Audio Markup'),
    );
  }
}

class MyHomePage extends StatefulWidget {
  const MyHomePage({super.key, required this.title});

  final String title;

  @override
  State<MyHomePage> createState() => _MyHomePageState();
}

class _MyHomePageState extends State<MyHomePage> {
  final player = AudioPlayer()..setReleaseMode(ReleaseMode.loop);
  String fileName = "";
  Duration duration = Duration.zero;
  Duration position = Duration.zero;
  int changed = 0;
  int add = 0;
  double volume = 1.0;
  int posInList = -1;
  String splitResult = "";
  Duration splitCompleted = Duration.zero;

  int selectedRow = 0;
  int selectedColumn = 0;

  final _editableKey = GlobalKey<EditableState>(); 
  List headers = [
    {"title":'Pos', 'widthFactor': 0.1, 'key':'pos'},
    {"title":'Artist', 'widthFactor': 0.4, 'key':'artist'},
    {"title":'Title', 'widthFactor': 0.45, 'key':'title'},
  ];
  List rows = [
    {"pos": '00:00', "artist":'',"title":''},
  ];

  @override
  void initState() {
    AudioLogger.logLevel = AudioLogLevel.info;

    player.onDurationChanged.listen((Duration d) {
      setState(() => duration = d);
    });

    player.onPositionChanged.listen((Duration  p) {
      //print('Current position: $p');
      setState(() => position = p);
      if (add != 0) {
        changed++;
      }
      if (changed >= 2) {
        print("> $changed add $add pos $p");
        if (p.inSeconds > add * 10 && add < 0) {
          player.seek(p - Duration(seconds: -add * 10));
        } else if (add > 0) {
          player.seek(p + Duration(seconds: add * 10));
        }
        changed = 0;
        add = 0;
      }
    });

    player.onLog.listen(
      (String message) => print("onLog: ${message}"),
      onError: (Object e, [StackTrace? stackTrace]) {print(e); print(stackTrace);},
    );

    super.initState();
  }

  String durationToText(Duration d) {
    return "${d.inMinutes.toString().padLeft(2, '0')}:${(d.inSeconds % 60).toString().padLeft(2, '0')}";
  }

  bool isNullOrEmpty(dynamic value) {
    if (value == null) return true;
    if (value is String && value.isEmpty) return true;
    return false;
  }

  String getTimestampForFFMPEG(String time) {
    //HH:MM:ss.ms
    var parts = time.split(':');
    int minutes = int.parse(parts[0]);
    String seconds = parts[1];
    int hours = (minutes / 60).floor();
    minutes = minutes % 60;
    String hs = hours.toString().padLeft(2,'0');
    String ms = minutes.toString().padLeft(2,'0');
    return '$hs:$ms:$seconds';
  }

  Future<String> split(String selectedDirectory) async {
    bool isMP3Format = fileName.endsWith('.mp3');

    String splitResult = "Success";

    for (int i = 0; i < rows.length; i++) {
      final row = rows[i];
      if (isNullOrEmpty(row['artist']) && isNullOrEmpty(row['title'])) {
        continue;
      }

      List<String> arguments = [];
      arguments.add('-i');
      arguments.add(fileName);
      arguments.add('-ss');
      arguments.add(getTimestampForFFMPEG(row['pos']));

      if (i + 1 < rows.length) {
        arguments.add('-to');
        arguments.add(getTimestampForFFMPEG(rows[i + 1]['pos']));
      }

      arguments.add('-metadata');
      arguments.add('title=${row['title']}');
      arguments.add('-metadata');
      arguments.add('artist=${row['artist']}');

      arguments.add('-acodec');
      if (isMP3Format) {
        arguments.add('copy');
      } else {
        arguments.add('libmp3lame');
        arguments.add('-q:a');
        arguments.add('2');
      }

      var name = '$selectedDirectory/${row['artist']} - ${row['title']}.mp3';
      arguments.add(name);
      
      ProcessResult result = await Process.run('ffmpeg', arguments, runInShell: true);

      // Logging the standard output (stdout)
      String out = result.stdout + result.stderr;
      
      // Checking if the process was successful
      if (result.exitCode != 0) {
          if (splitResult == "Success") {
            splitResult = "";
          }
          splitResult = splitResult + out;
      }
    }

    return splitResult.replaceAll('\n', ' ');
  }

  @override
  Widget build(BuildContext context) {
    return RawKeyboardListener(
      focusNode: FocusNode(),
      onKey: (RawKeyEvent event) {
        if (event.isAltPressed) {
          if (event.isKeyPressed(LogicalKeyboardKey.arrowUp)) {
            // Alt + Up: Play
            player.resume();
          } else if (event.isKeyPressed(LogicalKeyboardKey.arrowDown)) {
            // Alt + Down: Pause
            player.pause();
          } else if (event.isKeyPressed(LogicalKeyboardKey.arrowLeft)) {
            // Alt + Left: Previous
            changed = 0;
            add--;
          } else if (event.isKeyPressed(LogicalKeyboardKey.arrowRight)) {
            // Alt + Right: Next
            changed = 0;
            add++;
          } else if (event.isKeyPressed(LogicalKeyboardKey.keyX)) {
            setState(() {
              _editableKey.currentState?.updateValue(posInList, "pos", durationToText(position));
            });
          } else if (event.isKeyPressed(LogicalKeyboardKey.keyS)) {
            var parts = rows[posInList]['pos'].split(':');
            int minutes = int.parse(parts[0]);
            int seconds = int.parse(parts[1]);
            player.seek(Duration(minutes: minutes, seconds: seconds));
          }
        }
      },
      child: Scaffold(
        appBar: AppBar(
          backgroundColor: Theme.of(context).colorScheme.inversePrimary,
          title: Text("${widget.title}: $fileName"),
        ),
        body: Center(
          child: Column(children: <Widget>[
              Row(children: [
                Text("${durationToText(position)} from ${durationToText(duration)}"),
                ButtonItem("Open", () async {
                  final result = await FilePicker.platform.pickFiles();
                  final path = result?.files.single.path;
                  if (path != null) {
                    await player.play(DeviceFileSource(path));
                    setState(() {
                      fileName = path;
                    });
                  }
                }),
                ButtonItem("Play", () async {
                  await player.resume();
                }),
                ButtonItem("Pause", () async {
                  await player.pause();
                }),
                ButtonItem("Prev", () {
                  changed = 0;
                  add--;
                }),
                ButtonItem("Next", () {
                  changed = 0;
                  add++;
                }),
                ButtonItem("Add", () {
                  setState(() {
                    _editableKey.currentState?.createRow();
                  });
                }),
                ButtonItem("Split", () async {
                  String? selectedDirectory = await FilePicker.platform.getDirectoryPath();
                  if (selectedDirectory != null) {
                    String output = await split(selectedDirectory);

                    setState(() {
                      splitResult = output;
                      splitCompleted = position;
                    });
                  }
               }),
                Slider(
                  value: volume,
                  onChanged: (newVolume) async {
                    // set player volume here using your player's setVolume or equivalent method
                    player.setVolume(newVolume);
                    setState(() {
                      volume = newVolume;  
                    });
                  },
                  min: 0.0,
                  max: 1.0,
                  divisions: 10,
                  label: "${(volume * 100).toInt()}%",
                ),
                SplitStatus(splitResult: splitResult, passed: position - splitCompleted),
              ]),

              Expanded(
                child: Editable(
                  key: _editableKey,
                  columns: headers,
                  columnCount: 3,
                  rows: rows,
                  showCreateButton: false,
                  tdStyle: const TextStyle(fontSize: 20),
                  showSaveIcon: false,
                  borderColor: Colors.grey.shade300,
                  currentIndex: posInList,
                  currentIndexColor: Colors.lime.shade100,
                  onTap: (index, col) {
                    setState(() {
                      posInList = index;
                    });
                  },
                  onValueChange: (rows, editedRows, index, col, value) {
                    var result = editedRows.indexWhere((element) {
                      return element['row'] != index ? false : true;
                    });

                    if (col == "artist" && value.contains("-")) {
                      final splitted = value.split('-');
                      final artist = splitted[0].trim();
                      final title = splitted[1].trim();

                      rows![index]['artist'] = artist;
                      rows[index]['title'] = title;

                      if (result != -1) {
                        editedRows[result]['artist'] = artist;
                        editedRows[result]['title'] = title;
                      } else {
                        editedRows.add({'row': index, 'artist': artist});
                        editedRows.add({'row': index, 'title': title});
                      }
                    } else {
                      rows![index][col]= value;  

                      if (result != -1) {
                        editedRows[result][col] = value;
                      } else {
                        editedRows.add({'row': index, col: value});
                      }
                    }
                  },
                )
              )
            ],
          ),
        ),
      ),
    );
  }
}

class ButtonItem extends StatefulWidget {
  final String label;
  final void Function() onTap;

  const ButtonItem(this.label, this.onTap, {super.key});

  @override
  _HoverAnimatedButtonState createState() => _HoverAnimatedButtonState();
}

class _HoverAnimatedButtonState extends State<ButtonItem> {
  bool _isHovering = false;
  
  @override
  Widget build(BuildContext context) {
    return MouseRegion(
      onEnter: (_) => setState(() => _isHovering = true),
      onExit: (_) => setState(() => _isHovering = false),
      child: GestureDetector(
        onTap: widget.onTap,
        child: Container(
            margin: const EdgeInsets.symmetric(horizontal: 5.0, vertical: 5.0),
            padding: const EdgeInsets.all(8.0),
            decoration: BoxDecoration(
              color: _isHovering ? const Color.fromARGB(207, 144, 171, 209) : const Color.fromARGB(207, 199, 219, 247),
              border: Border.all(color: Colors.blue, width: 2),
              borderRadius: BorderRadius.circular(8.0),
              boxShadow: const [BoxShadow(blurRadius: 2.0, spreadRadius:1, offset: Offset(2.0, 2.0), color: Color.fromARGB(80, 0, 0, 0))]),
          child: Text(widget.label, style: const TextStyle(fontSize: 12.0))
        ),
      )
    );
  }
}

class SplitStatus extends StatefulWidget {
  final String splitResult;
  final Duration passed;

  const SplitStatus({super.key, required this.splitResult, required this.passed});

  @override
  _SplitStatusState createState() => _SplitStatusState();
}

class _SplitStatusState extends State<SplitStatus> {
  final TextEditingController _controller = TextEditingController();
  
  @override
  Widget build(BuildContext context) {
    if (widget.splitResult == "" || (widget.splitResult == "Success" && widget.passed.inSeconds > 15)) {
      return Container();
    }

    if (widget.splitResult == "Success") {
      return const Text("Split is completed");
    }
    
    if (_controller.text != widget.splitResult) {
      _controller.text = widget.splitResult;
    }
    

    return SizedBox(width: 200, child: TextFormField(controller: _controller, maxLines: 1)) ;
  }
}