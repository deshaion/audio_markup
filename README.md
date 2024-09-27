# audio_markup

## The issue with seeking the position

Go to audio_player.cc file in /home/ivan/.pub-cache/hosted/pub.dev/audioplayers_linux-4.0.0/linux/
Check your version and change audiosink

```
if (panorama) {
        audiobin = gst_bin_new(NULL);
        this->OnLog("alsasink initialized");
        audiosink = gst_element_factory_make("alsasink", NULL);
```

the old version is
```
audiosink = gst_element_factory_make("autoaudiosink", NULL);
```