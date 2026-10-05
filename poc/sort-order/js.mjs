const S = ["add x","Bump y","fix z","Zebra","apple","Äpfel","éclair","eclair","Eclair","ünicode","uber","Über","ß straße","ss","10 items","2 items","_private","-dash","Ångström","angstrom","naïve","naive","résumé","resume","co-op","coop","Coop","日本","中文","ёлка","елка","Ёж","a","A","b","B"];
console.log("JS\t" + [...S].sort((a,b)=>a.localeCompare(b)).join(" | "));
console.log("JS_EN\t" + [...S].sort((a,b)=>a.localeCompare(b,"en")).join(" | "));
