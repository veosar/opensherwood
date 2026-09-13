// Exports every defined string with the functions that reference it to re/out/strings.tsv (ADR-0009 analyst
// tooling; the output stays in the ignored re/). Columns: address, length, referencing function addresses, text.
//@category OpenSherwood
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.*;
import ghidra.program.model.symbol.*;
import java.io.*;
import java.util.*;

public class ExportStrings extends GhidraScript {
    @Override
    public void run() throws Exception {
        String[] args = getScriptArgs();
        File outDir = new File(args.length > 0 ? args[0] : "re/out");
        outDir.mkdirs();
        ReferenceManager rm = currentProgram.getReferenceManager();
        FunctionManager fm = currentProgram.getFunctionManager();
        try (PrintWriter w = new PrintWriter(new FileWriter(new File(outDir, "strings.tsv")))) {
            w.println("address\tlength\tfunctions\ttext");
            DataIterator it = currentProgram.getListing().getDefinedData(true);
            while (it.hasNext() && !monitor.isCancelled()) {
                Data d = it.next();
                if (!d.hasStringValue()) continue;
                Object v = d.getValue();
                if (v == null) continue;
                String text = v.toString().replace('\t', ' ').replace('\n', ' ').replace('\r', ' ');
                Set<String> funcs = new TreeSet<>();
                for (Reference r : rm.getReferencesTo(d.getAddress())) {
                    Function f = fm.getFunctionContaining(r.getFromAddress());
                    if (f != null) funcs.add(f.getEntryPoint().toString());
                }
                w.println(d.getAddress() + "\t" + text.length() + "\t" + String.join(",", funcs) + "\t" + text);
            }
        }
        println("strings written to " + outDir);
    }
}
