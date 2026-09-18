// Writes the instruction listing (address, mnemonic, operands, and the decompiler-independent p-code count)
// of the functions listed (one hex address per line) in the file given as the first argument to
// <out dir>/listing/<address>.asm, for the cases where the decompiler drops operations (x87 floating point
// in MSVC 6 code). ADR-0009 analyst tooling; the output stays under the analysis root `re/`.
//@category OpenSherwood
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.*;
import java.io.*;
import java.nio.file.*;

public class ExportListing extends GhidraScript {
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
        if (args.length < 1) { println("usage: ExportListing <list file> [out dir]"); return; }
        File dir = new File(analysisOut(args, 1), "listing");
        dir.mkdirs();
        FunctionManager fm = currentProgram.getFunctionManager();
        Listing listing = currentProgram.getListing();
        for (String line : Files.readAllLines(Paths.get(args[0]))) {
            String s = line.trim();
            if (s.isEmpty() || s.startsWith("#")) continue;
            if (s.startsWith("0x")) s = s.substring(2);
            Address a = currentProgram.getAddressFactory().getDefaultAddressSpace().getAddress(Long.parseLong(s, 16));
            Function f = fm.getFunctionContaining(a);
            if (f == null) { println("no function at " + s); continue; }
            try (PrintWriter w = new PrintWriter(new FileWriter(new File(dir, f.getEntryPoint() + ".asm")))) {
                w.println("; " + f.getName() + " at " + f.getEntryPoint() + " size " + f.getBody().getNumAddresses());
                InstructionIterator it = listing.getInstructions(f.getBody(), true);
                while (it.hasNext() && !monitor.isCancelled()) {
                    Instruction ins = it.next();
                    w.println(ins.getAddress() + "  " + ins.toString());
                }
            }
        }
        if (monitor.isCancelled()) throw new IOException("cancelled: the output is incomplete");
        println("listing written to " + dir);
    }
}
