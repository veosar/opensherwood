// Exports every defined string with the functions that reference it to re/out/strings.tsv (ADR-0009 analyst
// tooling; the output stays in the ignored re/). Columns: address, length, referencing function addresses, text.
//@category OpenSherwood
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.*;
import ghidra.program.model.listing.*;
import ghidra.program.model.symbol.*;
import java.io.*;
import java.util.*;

public class ExportStrings extends GhidraScript {

    /** The output directory must be inside the analysis root (a directory named `re`); anything else is refused. */
    static File analysisOut(String[] args, int index) throws IOException {
        File out = new File(args.length > index ? args[index] : "re/out").getCanonicalFile();
        for (File p = out; p != null; p = p.getParentFile()) {
            if (p.getName().equals("re")) { out.mkdirs(); return out; }
        }
        throw new IOException("refusing to write outside the analysis root (a directory named re): " + out);
    }

    @Override
    public void run() throws Exception {
        String[] args = getScriptArgs();
        File outDir = analysisOut(args, 0);
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
        if (monitor.isCancelled()) throw new IOException("cancelled: the output is incomplete");
        println("strings written to " + outDir);
    }
}
