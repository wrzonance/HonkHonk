SIZES        = 16 22 24 32 48 64 128 256 512
APPID        = io.github.wrzonance.HonkHonk
SVG_COLOR    = icon.svg
SVG_SYMBOLIC = icon-symbolic.svg
OUT          = generated

RESVG  ?= resvg
MAGICK ?= magick

# Build explicit size-x-size directory targets:
#   generated/hicolor/16x16/apps/io.github.wrzonance.HonkHonk.png, etc.
# A `foreach`+`eval` template emits one concrete rule per size — Make's
# pattern-rule syntax only honors the FIRST `%`, so SIZExSIZE paths
# cannot be expressed with a single static pattern.
HICOLOR_PNGS = $(foreach s,$(SIZES),$(OUT)/hicolor/$(s)x$(s)/apps/$(APPID).png)

.PHONY: icons hicolor symbolic favicon social clean check-tools

icons: check-tools hicolor symbolic favicon social

check-tools:
	@command -v $(RESVG)  >/dev/null || { echo "ERROR: resvg missing — cargo install resvg --locked"; exit 1; }
	@command -v $(MAGICK) >/dev/null || { echo "ERROR: ImageMagick missing"; exit 1; }

hicolor: $(HICOLOR_PNGS) $(OUT)/hicolor/scalable/apps/$(APPID).svg

define HICOLOR_PNG_RULE
$(OUT)/hicolor/$(1)x$(1)/apps/$(APPID).png: $(SVG_COLOR)
	@mkdir -p $$(dir $$@)
	$(RESVG) --width $(1) --height $(1) $$< $$@
endef
$(foreach s,$(SIZES),$(eval $(call HICOLOR_PNG_RULE,$(s))))

$(OUT)/hicolor/scalable/apps/$(APPID).svg: $(SVG_COLOR)
	@mkdir -p $(dir $@)
	cp $< $@

symbolic: $(OUT)/hicolor/symbolic/apps/$(APPID)-symbolic.svg

$(OUT)/hicolor/symbolic/apps/$(APPID)-symbolic.svg: $(SVG_SYMBOLIC)
	@mkdir -p $(dir $@)
	cp $< $@

favicon: $(OUT)/favicon.ico

$(OUT)/favicon.ico: $(SVG_COLOR)
	@mkdir -p $(OUT)
	$(RESVG) --width 256 --height 256 $< $(OUT)/favicon-256.png
	$(MAGICK) $(OUT)/favicon-256.png -background none \
	  -define icon:auto-resize="256,128,64,48,32,16" $@
	rm $(OUT)/favicon-256.png

social: $(OUT)/social-preview.png

$(OUT)/social-preview.png: $(SVG_COLOR)
	@mkdir -p $(OUT)
	$(RESVG) --width 512 --height 512 $< $(OUT)/social-icon.png
	$(MAGICK) -size 1280x640 xc:'#1a1a2e' \
	  $(OUT)/social-icon.png -gravity center -composite \
	  -font DejaVu-Sans-Bold -pointsize 96 -fill white \
	  -gravity south -annotate +0+80 'HonkHonk' $@
	rm $(OUT)/social-icon.png

clean:
	rm -rf $(OUT)
