import readline from "node:readline";
import tsaComposer from "tsa-composer";

// prompt user for input
export const prompt = async (question: string): Promise<string> => {
  const rl = readline.createInterface({
    input: process.stdin,
    output: process.stdout,
  });

  return new Promise((resolve) => {
    rl.question(question, (answer) => {
      rl.close();
      resolve(answer.trim());
    });
  });
};

export const promptT = tsaComposer()(prompt);
