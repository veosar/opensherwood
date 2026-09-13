// Exports a function inventory of the analysed program to re/out/inventory.tsv (ADR-0009 analyst tooling).
// Columns: address, size, name, callers, callees, referenced strings (count), first referenced string address.
// Generic: contains nothing from the analysed program. Run headless:
//   analyzeHeadless <project dir> robinhood -process "Robin Hood.exe" -noanalysis
//     -scriptPath <repo>/scripts/ghidra -postScript ExportInventory.java <out dir>
//@category OpenSherwood
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.*;
import ghidra.program.model.listing.*;
import ghidra.program.model.symbol.*;
import java.io.*;
import java.util.*;

public class ExportInventory extends GhidraScript {
    @Override
    public void run() throws Exception {
        String[] args = getScriptArgs();
        File outDir = new File(args.length > 0 ? args[0] : "re/out");
        outDir.mkdirs();
        try (PrintWriter w = new PrintWriter(new FileWriter(new File(outDir, "inventory.tsv")))) {
            w.println("address\tsize\tname\tcallers\tcallees\tstring_refs\tfirst_string");
            FunctionIterator it = currentProgram.getFunctionManager().getFunctions(true);
            ReferenceManager rm = currentProgram.getReferenceManager();
            while (it.hasNext() && !monitor.isCancelled()) {
                Function f = it.next();
                long size = f.getBody().getNumAddresses();
                int callers = f.getCallingFunctions(monitor).size();
                int callees = f.getCalledFunctions(monitor).size();
                int strings = 0;
                String first = "";
                AddressIterator ai = f.getBody().getAddresses(true);
                while (ai.hasNext()) {
                    Address a = ai.next();
                    for (Reference r : rm.getReferencesFrom(a)) {
                        Data d = currentProgram.getListing().getDefinedDataAt(r.getToAddress());
                        if (d != null && d.hasStringValue()) {
                            strings++;
                            if (first.isEmpty()) first = r.getToAddress().toString();
                        }
                    }
                }
                w.println(f.getEntryPoint() + "\t" + size + "\t" + f.getName() + "\t" + callers + "\t" + callees
                        + "\t" + strings + "\t" + first);
            }
        }
        println("inventory written to " + outDir);
    }
}
