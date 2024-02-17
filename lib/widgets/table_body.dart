import 'package:flutter/material.dart';

class RowBuilder extends StatefulWidget {
  ///Builds row elements for the table
  /// its properties are not nullable
  const RowBuilder({
    Key? key,
    required this.tdAlignment,
    required this.tdStyle,
    required double trHeight,
    required Color borderColor,
    required double borderWidth,
    required this.cellData,
    required this.index,
    required this.col,
    required this.tdPaddingLeft,
    required this.tdPaddingTop,
    required this.tdPaddingBottom,
    required this.tdPaddingRight,
    required this.tdEditableMaxLines,
    required this.onSubmitted,
    required this.onChanged,
    required this.onTap,
    required this.widthRatio,
    required this.isEditable,
    required this.stripeColor1,
    required this.stripeColor2,
    required this.zebraStripe,
    required this.focusedBorder,
    required this.currentIndex,
    required this.currentIndexColor,
  })   : _trHeight = trHeight,
        _borderColor = borderColor,
        _borderWidth = borderWidth,
        super(key: key);

  /// Table row height
  final double _trHeight;
  final Color _borderColor;
  final double _borderWidth;
  final cellData;
  final double? widthRatio;
  final bool isEditable;
  final TextAlign tdAlignment;
  final TextStyle? tdStyle;
  final int index;
  final col;
  final double tdPaddingLeft;
  final double tdPaddingTop;
  final double tdPaddingBottom;
  final double tdPaddingRight;
  final int tdEditableMaxLines;
  final Color stripeColor1;
  final Color stripeColor2;
  final bool zebraStripe;
  final InputBorder? focusedBorder;
  final ValueChanged<String>? onSubmitted;
  final ValueChanged<String> onChanged;
  final Function(int, String) onTap;
  final int? currentIndex;
  final Color? currentIndexColor;

  @override
  _RowBuilderState createState() => _RowBuilderState();
}

class _RowBuilderState extends State<RowBuilder> {
  final TextEditingController _controller = TextEditingController();

  @override
  Widget build(BuildContext context) {
    double width = MediaQuery.of(context).size.width;
    if (_controller.text != widget.cellData.toString()) {
      _controller.text = widget.cellData.toString();
      _controller.selection = TextSelection.fromPosition(
        TextPosition(offset: _controller.text.length),
      );
    }

    Color? color = !widget.zebraStripe ? null : (widget.index % 2 == 1.0 ? widget.stripeColor2 : widget.stripeColor1);
    if (widget.currentIndex != null && widget.currentIndex == widget.index) {
      color = widget.currentIndexColor;
    }

    return Flexible(
      fit: FlexFit.loose,
      flex: 6,
      child: Container(
        height: widget._trHeight < 40 ? 40 : widget._trHeight,
        width: width * widget.widthRatio!,
        decoration: BoxDecoration(
            color: color,
            border: Border.all(
                color: widget._borderColor, width: widget._borderWidth)),
        child: widget.isEditable
            ? TextFormField(
                textAlign: widget.tdAlignment,
                style: widget.tdStyle,
                controller: _controller,
                onFieldSubmitted: widget.onSubmitted,
                onChanged: widget.onChanged,
                onTap: () => widget.onTap(widget.index, widget.col),
                textAlignVertical: TextAlignVertical.center,
                maxLines: widget.tdEditableMaxLines,
                decoration: InputDecoration(
                  filled: widget.zebraStripe,
                  fillColor: widget.index % 2 == 1.0
                      ? widget.stripeColor2
                      : widget.stripeColor1,
                  contentPadding: EdgeInsets.only(
                      left: widget.tdPaddingLeft,
                      right: widget.tdPaddingRight,
                      top: widget.tdPaddingTop,
                      bottom: widget.tdPaddingBottom),
                  border: InputBorder.none,
                  focusedBorder: widget.focusedBorder,
                ),
              )
            : Container(
                alignment: Alignment.centerLeft,
                padding: EdgeInsets.only(
                  left: widget.tdPaddingLeft,
                  right: widget.tdPaddingRight,
                  // top: widget.tdPaddingTop,
                  // bottom: widget.tdPaddingBottom,
                ),
                decoration: BoxDecoration(
                  color: !widget.zebraStripe
                      ? null
                      : (widget.index % 2 == 1.0
                          ? widget.stripeColor2
                          : widget.stripeColor1),
                ),
                child: Text(
                  widget.cellData.toString(),
                  textAlign: widget.tdAlignment,
                  style: widget.tdStyle ??
                      const TextStyle(
                          // fontSize: Theme.of(context).textTheme.bodyText1.fontSize), // returns 14?
                          fontSize: 16),
                ),
              ),
      ),
    );
  }

  @override
  void dispose() {
    _controller.dispose(); // Dispose the TextEditingController when the state object is removed
    super.dispose();
  }
}
