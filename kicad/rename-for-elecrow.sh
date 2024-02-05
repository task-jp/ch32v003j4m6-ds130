#!/bin/sh

if [ $# -eq 0 ]; then
	echo "Usage: $0 <basename>"
	exit 1
fi

BASENAME=$1
rm -rf $BASENAME
rm -rf $BASENAME.zip
mkdir $BASENAME

declare -A rename_rules=(
	["-F_Cu.gtl"]=".GTL"
	["-B_Cu.gbl"]=".GBL"
	["-F_Mask.gts"]=".GTS"
	["-B_Mask.gbs"]=".GBS"
	["-F_Silkscreen.gto"]=".GTO"
	["-B_Silkscreen.gbo"]=".GBO"
	["-Edge_Cuts.gm1"]=".GML"
	[".drl"]=".TXT"
	["-NPTH.drl"]="-NPTH.TXT"
	["-PTH.drl"]="-PTH.TXT"
)

for file in *; do
	for key in "${!rename_rules[@]}"; do
		if [[ $file == "${BASENAME}${key}" ]]; then
			cp "$file" "${BASENAME}/${file/$key/${rename_rules[$key]}}"
		fi
	done
done

zip -r $BASENAME.zip $BASENAME
